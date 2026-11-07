//! `inno_engine`: the engine's reflex and search collection pools
//! for Python training code. Batches are transported as little-endian `f32`
//! bytes plus per-row metadata; Python reshapes them with the row sizes.
#![forbid(unsafe_code)]

use std::sync::Mutex;

use inno_engine::observation::pack::{self, Counts, Source};
use inno_engine::observation::{
    FAMILY, GENERATION, GROUPS, HEADER, OPTION_FIELDS, REVISION, SCALARS, TOKEN_FIELDS,
};
use inno_engine::{
    Batch, Match, Net, PoolSearch, Prompt, ReflexPool, Rng, RootNoise, SearchBatch, SearchConfig,
    SearchPool, TRAINING_MODES, mode_expansions,
};
use pyo3::buffer::{PyBuffer, ReadOnlyCell};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyDict};

/// Observations (all rows concatenated), per-row sizes, games, players,
/// option counts and finished games `(game, decisions, capped, winner)`.
type PyBatch<'py> = (
    Bound<'py, PyByteArray>,
    Vec<usize>,
    Vec<usize>,
    Vec<usize>,
    Vec<usize>,
    Vec<(usize, usize, bool, i32)>,
);

#[pyclass(name = "ReflexPool")]
struct PyReflexPool {
    pool: Mutex<ReflexPool>,
}

#[pymethods]
impl PyReflexPool {
    #[new]
    fn new(workers: usize) -> PyResult<Self> {
        Ok(Self {
            pool: Mutex::new(ReflexPool::new(workers).map_err(PyValueError::new_err)?),
        })
    }

    /// Start one match per seed; `subgames` plays Martian Internet's printed
    /// subgame instead of the draw variant (duels only; training keeps the
    /// variant its search values).
    #[pyo3(signature = (seeds, modes, decision_cap, subgames=false))]
    fn reset<'py>(
        &self,
        py: Python<'py>,
        seeds: Vec<u64>,
        modes: Vec<u8>,
        decision_cap: usize,
        subgames: bool,
    ) -> PyResult<PyBatch<'py>> {
        let batch = py
            .detach(|| {
                self.pool
                    .lock()
                    .map_err(|e| e.to_string())?
                    .reset_with_subgames(&seeds, &modes, decision_cap, subgames)
            })
            .map_err(PyValueError::new_err)?;
        transport(py, batch)
    }

    /// Apply one choice per active row; `seeds`/`modes` start new games
    /// (indices continue from the last started game) before observing.
    #[pyo3(signature = (choices, seeds=None, modes=None))]
    fn step<'py>(
        &self,
        py: Python<'py>,
        choices: Vec<usize>,
        seeds: Option<Vec<u64>>,
        modes: Option<Vec<u8>>,
    ) -> PyResult<PyBatch<'py>> {
        let seeds = seeds.unwrap_or_default();
        let modes = modes.unwrap_or_default();
        let batch = py
            .detach(|| {
                self.pool
                    .lock()
                    .map_err(|e| e.to_string())?
                    .step_with(&choices, &seeds, &modes)
            })
            .map_err(PyValueError::new_err)?;
        transport(py, batch)
    }

    fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            self.pool.lock().map_err(|e| e.to_string())?.close();
            Ok::<_, String>(())
        })
        .map_err(PyRuntimeError::new_err)
    }
}

fn transport(py: Python<'_>, batch: Batch) -> PyResult<PyBatch<'_>> {
    let sizes: Vec<usize> = batch.rows.iter().map(|r| r.observation.len()).collect();
    let obs = PyByteArray::new_with(py, sizes.iter().sum::<usize>() * 4, |out| {
        let (chunks, rest) = out.as_chunks_mut::<4>();
        debug_assert!(rest.is_empty());
        for (dst, value) in chunks
            .iter_mut()
            .zip(batch.rows.iter().flat_map(|row| &row.observation))
        {
            *dst = value.to_le_bytes();
        }
        Ok(())
    })?;
    Ok((
        obs,
        sizes,
        batch.rows.iter().map(|row| row.game).collect(),
        batch.rows.iter().map(|row| row.player).collect(),
        batch.rows.iter().map(|row| row.options).collect(),
        batch
            .finished
            .into_iter()
            .map(|g| (g.game, g.decisions, g.capped, g.winner))
            .collect(),
    ))
}

#[pyclass(name = "SearchPool")]
struct PySearchPool {
    pool: Mutex<SearchPool>,
}

/// Learned-search settings; `noise_alpha` adds Dirichlet root noise and
/// `virtual_loss` is the value counted against a path while its leaf waits.
#[allow(clippy::too_many_arguments)]
fn pool_search(
    iterations: u32,
    exploration: f64,
    prior_temperature: f64,
    leaf_batch: usize,
    noise_alpha: Option<f64>,
    noise_fraction: f64,
    virtual_loss: f64,
) -> PoolSearch {
    PoolSearch {
        config: SearchConfig {
            exploration,
            prior_temperature,
            root_noise: noise_alpha.map(|alpha| RootNoise {
                alpha,
                fraction: noise_fraction,
            }),
            virtual_loss,
            ..SearchConfig::learned(iterations)
        },
        leaf_batch,
    }
}

/// Learned information-set search over many games. Each batch holds leaf
/// observations that need Q vectors and searched roots that need a choice;
/// `step` takes both answers and waits for every game, while `collect`
/// returns whatever has reported and `answer` answers the oldest collected
/// batch without waiting, so one batch can be scored while the next is
/// collected.
#[pymethods]
impl PySearchPool {
    #[new]
    fn new(workers: usize) -> PyResult<Self> {
        Ok(Self {
            pool: Mutex::new(SearchPool::new(workers).map_err(PyValueError::new_err)?),
        })
    }

    #[pyo3(signature = (seeds, modes, decision_cap, iterations, exploration=1.5,
        prior_temperature=0.1, leaf_batch=8, noise_alpha=None, noise_fraction=0.25, virtual_loss=1.0))]
    #[allow(clippy::too_many_arguments)]
    fn reset<'py>(
        &self,
        py: Python<'py>,
        seeds: Vec<u64>,
        modes: Vec<u8>,
        decision_cap: usize,
        iterations: u32,
        exploration: f64,
        prior_temperature: f64,
        leaf_batch: usize,
        noise_alpha: Option<f64>,
        noise_fraction: f64,
        virtual_loss: f64,
    ) -> PyResult<Bound<'py, PyDict>> {
        let search = pool_search(
            iterations,
            exploration,
            prior_temperature,
            leaf_batch,
            noise_alpha,
            noise_fraction,
            virtual_loss,
        );
        let batch = py
            .detach(|| {
                self.pool.lock().map_err(|e| e.to_string())?.reset(
                    &seeds,
                    &modes,
                    decision_cap,
                    search,
                )
            })
            .map_err(PyValueError::new_err)?;
        search_transport(py, batch)
    }

    /// Settings for searches that start from now on.
    #[pyo3(signature = (iterations, exploration=1.5, prior_temperature=0.1, leaf_batch=8,
        noise_alpha=None, noise_fraction=0.25, virtual_loss=1.0))]
    #[allow(clippy::too_many_arguments)]
    fn set_search(
        &self,
        iterations: u32,
        exploration: f64,
        prior_temperature: f64,
        leaf_batch: usize,
        noise_alpha: Option<f64>,
        noise_fraction: f64,
        virtual_loss: f64,
    ) -> PyResult<()> {
        self.pool
            .lock()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?
            .set_search(pool_search(
                iterations,
                exploration,
                prior_temperature,
                leaf_batch,
                noise_alpha,
                noise_fraction,
                virtual_loss,
            ))
            .map_err(PyValueError::new_err)
    }

    /// `q`: every leaf's Q vector of the last batch, concatenated (a float32
    /// buffer or a sequence); `choices`: one option per root of the last
    /// batch; `seeds`/`modes` start new games before searching on.
    #[pyo3(signature = (q, choices, seeds=None, modes=None))]
    fn step<'py>(
        &self,
        py: Python<'py>,
        q: &Bound<'_, PyAny>,
        choices: Vec<usize>,
        seeds: Option<Vec<u64>>,
        modes: Option<Vec<u8>>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let q = row_floats(py, q)?;
        let seeds = seeds.unwrap_or_default();
        let modes = modes.unwrap_or_default();
        let batch = py
            .detach(|| {
                self.pool
                    .lock()
                    .map_err(|e| e.to_string())?
                    .step(&q, &choices, &seeds, &modes)
            })
            .map_err(PyValueError::new_err)?;
        search_transport(py, batch)
    }

    /// `step`'s answers for the oldest collected batch, without waiting: the
    /// games search on while the caller works, and report to `collect`.
    #[pyo3(signature = (q, choices, seeds=None, modes=None))]
    fn answer(
        &self,
        py: Python<'_>,
        q: &Bound<'_, PyAny>,
        choices: Vec<usize>,
        seeds: Option<Vec<u64>>,
        modes: Option<Vec<u8>>,
    ) -> PyResult<()> {
        let q = row_floats(py, q)?;
        let seeds = seeds.unwrap_or_default();
        let modes = modes.unwrap_or_default();
        py.detach(|| {
            self.pool
                .lock()
                .map_err(|e| e.to_string())?
                .answer(&q, &choices, &seeds, &modes)
        })
        .map_err(PyValueError::new_err)
    }

    /// The games that have reported, once their leaf and root rows number at
    /// least `min_rows` or no game is searching; `None` waits for every game.
    #[pyo3(signature = (min_rows=None))]
    fn collect<'py>(
        &self,
        py: Python<'py>,
        min_rows: Option<usize>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let batch = py
            .detach(|| {
                self.pool
                    .lock()
                    .map_err(|e| e.to_string())?
                    .collect(min_rows)
            })
            .map_err(PyValueError::new_err)?;
        search_transport(py, batch)
    }

    fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            self.pool.lock().map_err(|e| e.to_string())?.close();
            Ok::<_, String>(())
        })
        .map_err(PyRuntimeError::new_err)
    }
}

/// Little-endian float32 bytes of rows laid end to end.
fn float_bytes<'py, 'a>(
    py: Python<'py>,
    rows: impl Iterator<Item = &'a [f32]> + Clone,
) -> PyResult<Bound<'py, PyByteArray>> {
    let total: usize = rows.clone().map(<[f32]>::len).sum();
    PyByteArray::new_with(py, total * 4, |out| {
        let (chunks, _) = out.as_chunks_mut::<4>();
        for (dst, value) in chunks.iter_mut().zip(rows.flatten()) {
            *dst = value.to_le_bytes();
        }
        Ok(())
    })
}

/// A search batch as a dict: `leaf_obs` (bytes), `leaf_sizes`, `leaf_games`,
/// `leaf_options`; `root_obs`, `root_sizes`, `root_games`, `root_players`,
/// `root_options`, flat per-option `root_visits`, `root_values` and
/// `root_priors`, `root_searched`, `root_refused`; and `finished` as `(game, decisions,
/// capped, winner)`.
fn search_transport(py: Python<'_>, batch: SearchBatch) -> PyResult<Bound<'_, PyDict>> {
    let out = PyDict::new(py);
    let leaves = &batch.leaves;
    let roots = &batch.roots;
    out.set_item(
        "leaf_obs",
        float_bytes(py, leaves.iter().map(|l| &l.observation[..]))?,
    )?;
    out.set_item(
        "leaf_sizes",
        leaves
            .iter()
            .map(|l| l.observation.len())
            .collect::<Vec<_>>(),
    )?;
    out.set_item(
        "leaf_games",
        leaves.iter().map(|l| l.game).collect::<Vec<_>>(),
    )?;
    out.set_item(
        "leaf_options",
        leaves.iter().map(|l| l.options).collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_obs",
        float_bytes(py, roots.iter().map(|r| &r.observation[..]))?,
    )?;
    out.set_item(
        "root_sizes",
        roots
            .iter()
            .map(|r| r.observation.len())
            .collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_games",
        roots.iter().map(|r| r.game).collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_players",
        roots.iter().map(|r| r.player).collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_options",
        roots.iter().map(|r| r.options).collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_visits",
        roots
            .iter()
            .flat_map(|r| r.visits.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_values",
        roots
            .iter()
            .flat_map(|r| r.values.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_priors",
        roots
            .iter()
            .flat_map(|r| r.priors.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_searched",
        roots.iter().map(|r| r.searched).collect::<Vec<_>>(),
    )?;
    out.set_item(
        "root_refused",
        roots.iter().map(|r| r.refused).collect::<Vec<_>>(),
    )?;
    out.set_item(
        "finished",
        batch
            .finished
            .iter()
            .map(|g| (g.game, g.decisions, g.capped, g.winner))
            .collect::<Vec<_>>(),
    )?;
    Ok(out)
}

/// The observation row and option count for the player to move after
/// `steps` random steps of a fresh match in training mode `mode`.
#[pyfunction]
fn observe_random_root(mode: u8, seed: u64, steps: usize) -> PyResult<(Vec<f32>, usize)> {
    let expansions = mode_expansions(mode)
        .ok_or_else(|| PyValueError::new_err(format!("training mode {mode} is not in 0..32")))?;
    let mut session =
        Match::new(expansions, seed).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let mut policy = Rng::new(seed);
    for _ in 0..steps {
        let step = session
            .random_step(policy.next_u64())
            .ok_or_else(|| PyValueError::new_err("the game ended before the requested step"))?;
        session
            .apply(step)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    }
    let (player, options) = match session.prompt() {
        Prompt::Decision(decision) => (decision.player, decision.options.len()),
        Prompt::Turn { player, actions } => (*player, actions.len()),
        Prompt::Over(_) => {
            return Err(PyValueError::new_err(
                "the game ended before the requested step",
            ));
        }
    };
    let observation = session
        .game()
        .observe(player)
        .map_err(|e| PyRuntimeError::new_err(format!("{e:?}")))?;
    debug_assert_eq!(observation.options.len(), options);
    Ok((observation.row(), options))
}

/// Float32 buffer memory read in place, without a host copy.
struct Cells<'a>(&'a [ReadOnlyCell<f32>]);

impl Source for Cells<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    fn at(&self, index: usize) -> f32 {
        self.0[index].get()
    }
    #[inline]
    fn chunk<const N: usize>(&self, index: usize) -> [f32; N] {
        let cells = &self.0[index..index + N];
        std::array::from_fn(|i| cells[i].get())
    }
}

/// Work generic over how the rows are read, so each buffer kind gets its
/// own monomorphized pass.
trait Job {
    type Out;
    fn run<S: Source>(self, rows: S) -> Result<Self::Out, String>;
}

/// Run `job` over `flat` as float32 rows: a float32 buffer (NumPy array,
/// memoryview) is read in place, a byte buffer (bytes, bytearray) is decoded
/// as little-endian float32 first.
fn with_rows<J: Job>(py: Python<'_>, flat: &Bound<'_, PyAny>, job: J) -> PyResult<J::Out> {
    let result = if let Ok(buffer) = PyBuffer::<f32>::get(flat) {
        match buffer.as_slice(py) {
            Some(cells) => job.run(Cells(cells)),
            None => job.run(&buffer.to_vec(py)?[..]),
        }
    } else {
        let bytes = PyBuffer::<u8>::get(flat)
            .map_err(|_| PyValueError::new_err("rows must be a float32 or byte buffer"))?
            .to_vec(py)?;
        let (chunks, rest) = bytes.as_chunks::<4>();
        if !rest.is_empty() {
            return Err(PyValueError::new_err(
                "row bytes are not a whole number of float32 values",
            ));
        }
        let floats: Vec<f32> = chunks.iter().map(|b| f32::from_le_bytes(*b)).collect();
        job.run(&floats[..])
    };
    result.map_err(PyValueError::new_err)
}

type PyCounts = (usize, usize, usize, usize);

fn counts(c: Counts) -> PyCounts {
    (c.rows, c.tokens, c.records, c.options)
}

struct MeasureJob<'a> {
    sizes: &'a [usize],
}

impl Job for MeasureJob<'_> {
    type Out = PyCounts;
    fn run<S: Source>(self, rows: S) -> Result<PyCounts, String> {
        pack::measure(rows, self.sizes).map(counts)
    }
}

struct PackJob<'a> {
    sizes: &'a [usize],
    capacity: Counts,
    out: &'a [std::cell::Cell<f32>],
}

impl Job for PackJob<'_> {
    type Out = PyCounts;
    fn run<S: Source>(self, rows: S) -> Result<PyCounts, String> {
        pack::pack(rows, self.sizes, self.capacity, self.out).map(counts)
    }
}

/// Floats in a packed buffer with these capacities (see `pack_rows`).
#[pyfunction]
fn pack_width(b_cap: usize, n_cap: usize, r_cap: usize, k_cap: usize) -> usize {
    Counts {
        rows: b_cap,
        tokens: n_cap,
        records: r_cap,
        options: k_cap,
    }
    .width()
}

/// Validate `row_sizes` consecutive next-1 rows of `flat` and return
/// `(n_rows, n_pooled_tokens, n_records, n_options)`, the capacities packing
/// them needs.
#[pyfunction]
fn measure_rows(
    py: Python<'_>,
    flat: &Bound<'_, PyAny>,
    row_sizes: Vec<usize>,
) -> PyResult<PyCounts> {
    with_rows(py, flat, MeasureJob { sizes: &row_sizes })
}

/// Pack `row_sizes` consecutive next-1 rows of `flat` into `out`, a writable
/// C-contiguous float32 buffer of exactly `pack_width(b_cap, n_cap, r_cap,
/// k_cap)` floats, and return the used `(n_rows, n_tokens, n_records,
/// n_options)`. Layout: `b_cap × SCALARS` scalars, `b_cap` seats, six
/// pooled-token planes of `n_cap` (row, card, group, weight, code, value),
/// `r_cap × 56` records (row, card, kind, depth, present, 0, fields 1..=50),
/// `k_cap × 15` options (row, present, fields). Padding is zero.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn pack_rows(
    py: Python<'_>,
    flat: &Bound<'_, PyAny>,
    row_sizes: Vec<usize>,
    out: &Bound<'_, PyAny>,
    b_cap: usize,
    n_cap: usize,
    r_cap: usize,
    k_cap: usize,
) -> PyResult<PyCounts> {
    let buffer = PyBuffer::<f32>::get(out)
        .map_err(|_| PyValueError::new_err("out must be a float32 buffer"))?;
    let cells = buffer.as_mut_slice(py).ok_or_else(|| {
        PyValueError::new_err("out must be a writable C-contiguous float32 buffer")
    })?;
    with_rows(
        py,
        flat,
        PackJob {
            sizes: &row_sizes,
            capacity: Counts {
                rows: b_cap,
                tokens: n_cap,
                records: r_cap,
                options: k_cap,
            },
            out: cells,
        },
    )
}

/// One next-1 row as floats: a float32 buffer (NumPy array, memoryview) or
/// byte buffer is read as `with_rows` does; any other sequence of numbers is
/// converted element by element.
fn row_floats(py: Python<'_>, row: &Bound<'_, PyAny>) -> PyResult<Vec<f32>> {
    struct Collect;
    impl Job for Collect {
        type Out = Vec<f32>;
        fn run<S: Source>(self, rows: S) -> Result<Vec<f32>, String> {
            Ok((0..rows.len()).map(|i| rows.at(i)).collect())
        }
    }
    if PyBuffer::<f32>::get(row).is_ok() || PyBuffer::<u8>::get(row).is_ok() {
        return with_rows(py, row, Collect);
    }
    row.extract::<Vec<f32>>()
        .map_err(|_| PyValueError::new_err("row must be a float32 buffer or a sequence of floats"))
}

/// A next-1 network loaded from INNONET1/2 bytes (`train/export_td_weights.py`).
#[pyclass(name = "Net")]
struct PyNet {
    net: Net,
}

#[pymethods]
impl PyNet {
    #[new]
    fn new(data: &[u8]) -> PyResult<Self> {
        Ok(Self {
            net: Net::from_bytes(data).map_err(PyValueError::new_err)?,
        })
    }

    /// `(observation generation, revision)` the network consumes.
    fn contract(&self) -> (u32, u32) {
        self.net.contract()
    }

    /// The trainer iteration the export recorded.
    fn iteration(&self) -> u32 {
        self.net.iteration()
    }

    /// One Q per option of a next-1 row, in the row's option order.
    fn q(&self, py: Python<'_>, row: &Bound<'_, PyAny>) -> PyResult<Vec<f32>> {
        let row = row_floats(py, row)?;
        py.detach(|| self.net.q_row(&row))
            .map_err(PyValueError::new_err)
    }

    /// One policy logit per option of a next-1 row, or None for a network
    /// without a policy output.
    fn policy(&self, py: Python<'_>, row: &Bound<'_, PyAny>) -> PyResult<Option<Vec<f32>>> {
        let row = row_floats(py, row)?;
        py.detach(|| self.net.evaluate_row(&row))
            .map(|(_, policy)| policy)
            .map_err(PyValueError::new_err)
    }

    /// The index of the option with the highest Q (the first on ties), or
    /// None when the row has no options.
    fn greedy(&self, py: Python<'_>, row: &Bound<'_, PyAny>) -> PyResult<Option<usize>> {
        Ok(inno_engine::net::argmax(&self.q(py, row)?))
    }
}

#[pymodule(name = "inno_engine")]
fn extension(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyReflexPool>()?;
    m.add_class::<PySearchPool>()?;
    m.add_class::<PyNet>()?;
    m.add_function(wrap_pyfunction!(observe_random_root, m)?)?;
    m.add_function(wrap_pyfunction!(pack_width, m)?)?;
    m.add_function(wrap_pyfunction!(measure_rows, m)?)?;
    m.add_function(wrap_pyfunction!(pack_rows, m)?)?;
    m.add("RECORD_WIDTH", pack::RECORD_WIDTH)?;
    m.add("RECORD_FIELD_BASE", pack::RECORD_FIELD_BASE)?;
    m.add("OPTION_WIDTH", pack::OPTION_WIDTH)?;
    m.add("TOKEN_PLANES", pack::TOKEN_PLANES)?;
    m.add("TRAINING_MODES", TRAINING_MODES.to_vec())?;
    m.add("OBSERVATION_HEADER", FAMILY + GENERATION)?;
    m.add("OBSERVATION_REVISION", REVISION)?;
    m.add("HEADER", HEADER)?;
    m.add("SCALARS", SCALARS)?;
    m.add("TOKEN_FIELDS", TOKEN_FIELDS)?;
    m.add("OPTION_FIELDS", OPTION_FIELDS)?;
    m.add("GROUPS", GROUPS)?;
    Ok(())
}

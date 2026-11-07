//! Immutable public names from reference/figures-text.json. These are naming
//! choices, not a claim that all Figure effects are implemented.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FigureName {
    pub code: &'static str,
    pub title: &'static str,
    pub age: u8,
}

/// Original deck rosters never depend on current physical locations or belief.
pub static FIGURE_NAMES: [FigureName; 115] = [
    FigureName {
        code: "F001",
        title: "HAMMURABI",
        age: 1,
    },
    FigureName {
        code: "F002",
        title: "GILGAMESH",
        age: 1,
    },
    FigureName {
        code: "F003",
        title: "TIGERNMAS",
        age: 1,
    },
    FigureName {
        code: "F004",
        title: "SUNSHU AO",
        age: 1,
    },
    FigureName {
        code: "F005",
        title: "SHENNONG",
        age: 1,
    },
    FigureName {
        code: "F006",
        title: "SNEFERU",
        age: 1,
    },
    FigureName {
        code: "F007",
        title: "HATSHEPSUT",
        age: 1,
    },
    FigureName {
        code: "F008",
        title: "FU XI",
        age: 1,
    },
    FigureName {
        code: "F009",
        title: "SARGON OF AKKAD",
        age: 1,
    },
    FigureName {
        code: "F010",
        title: "DAEDALUS",
        age: 1,
    },
    FigureName {
        code: "F011",
        title: "IMHOTEP",
        age: 1,
    },
    FigureName {
        code: "F012",
        title: "HUANG DI",
        age: 1,
    },
    FigureName {
        code: "F013",
        title: "PTAHHOTEP",
        age: 1,
    },
    FigureName {
        code: "F014",
        title: "SINUHE",
        age: 1,
    },
    FigureName {
        code: "F015",
        title: "HOMER",
        age: 1,
    },
    FigureName {
        code: "F016",
        title: "SUN TZU",
        age: 2,
    },
    FigureName {
        code: "F017",
        title: "ALEXANDER THE GREAT",
        age: 2,
    },
    FigureName {
        code: "F018",
        title: "XIMEN BAO",
        age: 2,
    },
    FigureName {
        code: "F019",
        title: "CAI LUN",
        age: 2,
    },
    FigureName {
        code: "F020",
        title: "PTOLEMY",
        age: 2,
    },
    FigureName {
        code: "F021",
        title: "AUGUSTUS CAESAR",
        age: 2,
    },
    FigureName {
        code: "F022",
        title: "ARCHIMEDES",
        age: 2,
    },
    FigureName {
        code: "F023",
        title: "ZHANG HENG",
        age: 2,
    },
    FigureName {
        code: "F024",
        title: "CONFUCIUS",
        age: 2,
    },
    FigureName {
        code: "F025",
        title: "PLATO",
        age: 2,
    },
    FigureName {
        code: "F026",
        title: "TRAN HUANG DAO",
        age: 3,
    },
    FigureName {
        code: "F027",
        title: "SALADIN",
        age: 3,
    },
    FigureName {
        code: "F028",
        title: "AVICENNA",
        age: 3,
    },
    FigureName {
        code: "F029",
        title: "RHAZES",
        age: 3,
    },
    FigureName {
        code: "F030",
        title: "SHEN KUO",
        age: 3,
    },
    FigureName {
        code: "F031",
        title: "SU SONG",
        age: 3,
    },
    FigureName {
        code: "F032",
        title: "ALHAZEN",
        age: 3,
    },
    FigureName {
        code: "F033",
        title: "SEJONG THE GREAT",
        age: 3,
    },
    FigureName {
        code: "F034",
        title: "MURASAKI SHIKIBU",
        age: 3,
    },
    FigureName {
        code: "F035",
        title: "AL-KINDI",
        age: 3,
    },
    FigureName {
        code: "F036",
        title: "AMINA SUKHERA",
        age: 4,
    },
    FigureName {
        code: "F037",
        title: "YI SUN-SIN",
        age: 4,
    },
    FigureName {
        code: "F038",
        title: "LEONARDO DA VINCI",
        age: 4,
    },
    FigureName {
        code: "F039",
        title: "MICHELANGELO",
        age: 4,
    },
    FigureName {
        code: "F040",
        title: "GALILEO GALILEI",
        age: 4,
    },
    FigureName {
        code: "F041",
        title: "PEDRO ALVARES CABRAL",
        age: 4,
    },
    FigureName {
        code: "F042",
        title: "TAKI AD-DIN",
        age: 4,
    },
    FigureName {
        code: "F043",
        title: "JOHANNES KEPLER",
        age: 4,
    },
    FigureName {
        code: "F044",
        title: "NICCOLÒ MACHIAVELLI",
        age: 4,
    },
    FigureName {
        code: "F045",
        title: "WILLIAM SHAKESPEARE",
        age: 4,
    },
    FigureName {
        code: "F046",
        title: "SHIVAJI",
        age: 5,
    },
    FigureName {
        code: "F047",
        title: "PETER THE GREAT",
        age: 5,
    },
    FigureName {
        code: "F048",
        title: "ANTONIE VAN LEEUWENHOEK",
        age: 5,
    },
    FigureName {
        code: "F049",
        title: "CHRISTOPHER POLHEM",
        age: 5,
    },
    FigureName {
        code: "F050",
        title: "SAMUEL DE CHAMPLAIN",
        age: 5,
    },
    FigureName {
        code: "F051",
        title: "BARTHOLOMEW ROBERTS",
        age: 5,
    },
    FigureName {
        code: "F052",
        title: "ISAAC NEWTON",
        age: 5,
    },
    FigureName {
        code: "F053",
        title: "CHRISTIAAN HUYGENS",
        age: 5,
    },
    FigureName {
        code: "F054",
        title: "BARTOLOMEO CRISTOFORI",
        age: 5,
    },
    FigureName {
        code: "F055",
        title: "JOHANNES VERMEER",
        age: 5,
    },
    FigureName {
        code: "F056",
        title: "NAPOLEON BONAPARTE",
        age: 6,
    },
    FigureName {
        code: "F057",
        title: "TECUMSEH",
        age: 6,
    },
    FigureName {
        code: "F058",
        title: "JOHN LOUDON MCADAM",
        age: 6,
    },
    FigureName {
        code: "F059",
        title: "EDWARD JENNER",
        age: 6,
    },
    FigureName {
        code: "F060",
        title: "ADAM SMITH",
        age: 6,
    },
    FigureName {
        code: "F061",
        title: "JOHN HARRISON",
        age: 6,
    },
    FigureName {
        code: "F062",
        title: "CARL FRIEDRICH GAUSS",
        age: 6,
    },
    FigureName {
        code: "F063",
        title: "BENJAMIN FRANKLIN",
        age: 6,
    },
    FigureName {
        code: "F064",
        title: "CATHERINE THE GREAT",
        age: 6,
    },
    FigureName {
        code: "F065",
        title: "LUDWIG VAN BEETHOVEN",
        age: 6,
    },
    FigureName {
        code: "F066",
        title: "ROBERT E. LEE",
        age: 7,
    },
    FigureName {
        code: "F067",
        title: "JOHN ERICSSON",
        age: 7,
    },
    FigureName {
        code: "F068",
        title: "ROWLAND HILL",
        age: 7,
    },
    FigureName {
        code: "F069",
        title: "FLORENCE NIGHTINGALE",
        age: 7,
    },
    FigureName {
        code: "F070",
        title: "GEORGE STEPHENSON",
        age: 7,
    },
    FigureName {
        code: "F071",
        title: "ALFRED NOBEL",
        age: 7,
    },
    FigureName {
        code: "F072",
        title: "CHARLES DARWIN",
        age: 7,
    },
    FigureName {
        code: "F073",
        title: "JAMES CLERK MAXWELL",
        age: 7,
    },
    FigureName {
        code: "F074",
        title: "EMPEROR MEIJI",
        age: 7,
    },
    FigureName {
        code: "F075",
        title: "QUEEN VICTORIA",
        age: 7,
    },
    FigureName {
        code: "F076",
        title: "ERWIN ROMMEL",
        age: 8,
    },
    FigureName {
        code: "F077",
        title: "JOHN VON NEUMANN",
        age: 8,
    },
    FigureName {
        code: "F078",
        title: "CARESSE CROSBY",
        age: 8,
    },
    FigureName {
        code: "F079",
        title: "NIKOLA TESLA",
        age: 8,
    },
    FigureName {
        code: "F080",
        title: "J.P. MORGAN",
        age: 8,
    },
    FigureName {
        code: "F081",
        title: "EMMY NOETHER",
        age: 8,
    },
    FigureName {
        code: "F082",
        title: "MARIE CURIE",
        age: 8,
    },
    FigureName {
        code: "F083",
        title: "ALBERT EINSTEIN",
        age: 8,
    },
    FigureName {
        code: "F084",
        title: "H.G. WELLS",
        age: 8,
    },
    FigureName {
        code: "F085",
        title: "DUKE ELLINGTON",
        age: 8,
    },
    FigureName {
        code: "F086",
        title: "NELSON MANDELA",
        age: 9,
    },
    FigureName {
        code: "F087",
        title: "MIKHAIL KALASHNIKOV",
        age: 9,
    },
    FigureName {
        code: "F088",
        title: "RUTH HANDLER",
        age: 9,
    },
    FigureName {
        code: "F089",
        title: "CHE GUEVARA",
        age: 9,
    },
    FigureName {
        code: "F090",
        title: "HEDY LAMARR",
        age: 9,
    },
    FigureName {
        code: "F091",
        title: "TAIICHI OHNO",
        age: 9,
    },
    FigureName {
        code: "F092",
        title: "GRACE HOPPER",
        age: 9,
    },
    FigureName {
        code: "F093",
        title: "WERNHER VON BRAUN",
        age: 9,
    },
    FigureName {
        code: "F094",
        title: "PELÉ",
        age: 9,
    },
    FigureName {
        code: "F095",
        title: "ROD SERLING",
        age: 9,
    },
    FigureName {
        code: "F096",
        title: "JACKIE CHAN",
        age: 10,
    },
    FigureName {
        code: "F097",
        title: "MARGARET THATCHER",
        age: 10,
    },
    FigureName {
        code: "F098",
        title: "ALEX TREBEK",
        age: 10,
    },
    FigureName {
        code: "F099",
        title: "SHIGERU MIYAMOTO",
        age: 10,
    },
    FigureName {
        code: "F100",
        title: "MUHAMMAD YUNUS",
        age: 10,
    },
    FigureName {
        code: "F101",
        title: "SERGEY BRIN",
        age: 10,
    },
    FigureName {
        code: "F102",
        title: "SUSAN BLACKMORE",
        age: 10,
    },
    FigureName {
        code: "F103",
        title: "STEPHEN HAWKING",
        age: 10,
    },
    FigureName {
        code: "F104",
        title: "MARTIN SCORSESE",
        age: 10,
    },
    FigureName {
        code: "F105",
        title: "YUNA KIM",
        age: 10,
    },
    FigureName {
        code: "F106",
        title: "MAX VERSTAPPEN",
        age: 11,
    },
    FigureName {
        code: "F107",
        title: "SHEIKH TAMIM",
        age: 11,
    },
    FigureName {
        code: "F108",
        title: "GRETA THUNBERG",
        age: 11,
    },
    FigureName {
        code: "F109",
        title: "THERESE KIRONGOZI",
        age: 11,
    },
    FigureName {
        code: "F110",
        title: "BEAR GRYLLS",
        age: 11,
    },
    FigureName {
        code: "F111",
        title: "BOYAN SLAT",
        age: 11,
    },
    FigureName {
        code: "F112",
        title: "GIUSEPPE SCIONTI",
        age: 11,
    },
    FigureName {
        code: "F113",
        title: "KONSTANTIN NOVOSELOV",
        age: 11,
    },
    FigureName {
        code: "F114",
        title: "EMMA WATSON",
        age: 11,
    },
    FigureName {
        code: "F115",
        title: "LILY HEVESH",
        age: 11,
    },
];

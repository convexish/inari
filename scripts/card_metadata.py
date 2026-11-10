"""Generate the engine's printed-card tables from `data/*.json`.

    pixi run metadata           # rewrite engine/src/catalog/*_metadata.rs
    pixi run metadata --check   # fail if a table is out of date

`data/README.md` describes the schema every set shares. Each slot becomes
one `Symbol`; where `image_slot` names the image, other empty slots are blank.
Every set fills the same `CardMetadata` row; only Cities print six slots.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SLOTS = ("top_left", "bottom_left", "bottom_center", "bottom_right", "top_center", "top_right")
# Per data file: `CardSet`, card count, slots. Age 0 cards are Base cards.
SETS = {
    "base": ("Base", 115, 4),
    "echoes": ("Echoes", 115, 4),
    "cities": ("Cities", 115, 6),
    "unseen": ("Unseen", 115, 4),
    "artifacts": ("Artifacts", 115, 4),
    "figures": ("Figures", 115, 4),
    "age_zero": ("Base", 15, 4),
}


def variant(name: str) -> str:
    return name.capitalize()


def symbols(card: dict, slots: int) -> list[str]:
    code = card["code"]
    assert list(card["icons"]) == list(SLOTS[:slots]), code
    bonuses = card.get("bonuses", [None] * slots)
    city_icons = card.get("city_icons", [None] * slots)
    echoes = [echo["slot"] for echo in card.get("echoes", [])]
    assert len(bonuses) == slots and len(city_icons) == slots and len(echoes) <= 1, code
    out = []
    for slot, name in enumerate(SLOTS[:slots]):
        icon, bonus, city = card["icons"][name], bonuses[slot], city_icons[slot]
        printed = []
        if city == "search":
            assert icon is not None, (code, slot)
            printed.append(f"Symbol::Search(Icon::{variant(icon)})")
        else:
            if icon is not None:
                printed.append(f"Symbol::Icon(Icon::{variant(icon)})")
            if city is not None:
                printed.append(f"Symbol::City(CityIcon::{variant(city)})")
        if bonus is not None:
            printed.append(f"Symbol::Bonus({bonus})")
        if slot in echoes:
            printed.append("Symbol::Echo")
        assert len(printed) <= 1, (code, slot, printed)
        out.append(printed[0] if printed else None)
    empty = [slot for slot, symbol in enumerate(out) if symbol is None]
    image = card.get("image_slot", empty[0] if len(empty) == 1 else None)
    assert image in empty, (code, "image slot", empty)
    return ["Symbol::Image" if slot == image else symbol or "Symbol::Blank" for slot, symbol in enumerate(out)]


def featured(card: dict) -> str:
    icons = {effect["featured_icon"] for effect in card.get("effects", [])}
    assert len(icons) <= 1, card["code"]
    return f"Some(Icon::{variant(icons.pop())})" if icons else "None"


def table(name: str) -> str:
    card_set, count, slots = SETS[name]
    cards = json.loads((ROOT / f"data/{name}.json").read_text(encoding="utf-8"))
    codes = [card["code"] for card in cards]
    assert len(cards) == count and codes == sorted(set(codes)), name
    six = slots == len(SLOTS)
    body = "".join(
        "CardMetadata {"
        f"code: {json.dumps(card['code'])},"
        f"title: {json.dumps(card['title'], ensure_ascii=False)},"
        f"color: Color::{variant(card['color'])},"
        f"value: {card['age']},"
        f"set: CardSet::{card_set},"
        + ("symbols: [{}]," if six else "symbols: four_slots([{}]),").format(",".join(symbols(card, slots)))
        + f"featured: {featured(card)},"
        "},"
        for card in cards
    )
    imports = ["CardSet", "Color", "Symbol"]
    imports += [name for name in ("CityIcon", "Icon", "four_slots") if f"{name}::" in body or f"{name}(" in body]
    source = (
        f"// Generated from data/{name}.json by scripts/card_metadata.py; do not edit by hand.\n"
        "use super::CardMetadata;\n"
        f"use crate::model::{{{', '.join(sorted(imports, key=str.lower))}}};\n\n"
        f"pub const {name.upper()}_METADATA: [CardMetadata; {count}] = [{body}];\n"
    )
    return subprocess.run(
        ["rustfmt", "--edition", "2024"], input=source, capture_output=True, text=True, check=True
    ).stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="verify instead of writing")
    args = parser.parse_args()
    stale = []
    for name in SETS:
        target = ROOT / f"engine/src/catalog/{name}_metadata.rs"
        source = table(name)
        if args.check:
            if not target.exists() or target.read_text(encoding="utf-8") != source:
                stale.append(target.relative_to(ROOT))
        else:
            target.write_text(source, encoding="utf-8")
    for target in stale:
        print(f"out of date: {target}", file=sys.stderr)
    return 1 if stale else 0


if __name__ == "__main__":
    sys.exit(main())

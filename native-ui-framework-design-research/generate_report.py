from __future__ import annotations

import json
import re
import textwrap
from datetime import datetime
from pathlib import Path
from typing import Any

import yaml


ROOT = Path(__file__).resolve().parent
REPORT_PATH = ROOT / "report.md"
SUMMARY_FIELDS = [
    "open_gpui_relevance",
    "registry_viability",
    "must_have_for_open_gpui",
]

CATEGORY_MAPPING = {
    "基本信息": ["basic_info", "基本信息"],
    "技术特性": ["technical_features", "technical_characteristics", "技术特性"],
    "性能指标": ["performance_metrics", "performance", "性能指标"],
    "里程碑意义": ["milestone_significance", "milestones", "里程碑意义"],
    "商业信息": ["business_info", "commercial_info", "商业信息"],
    "竞争与生态": ["competition_ecosystem", "competition", "竞争与生态"],
    "历史沿革": ["history", "历史沿革"],
    "市场定位": ["market_positioning", "market", "市场定位"],
}


def load_yaml(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    return yaml.safe_load(path.read_text(encoding="utf-8")) or {}


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def resolve_results_dir(outline: dict[str, Any]) -> Path:
    candidates: list[str | None] = [
        outline.get("output_dir"),
        (outline.get("execution") or {}).get("output_dir"),
        "results",
    ]
    for candidate in candidates:
        if not candidate:
            continue
        path = Path(candidate)
        checks = [
            path if path.is_absolute() else Path.cwd() / path,
            path if path.is_absolute() else ROOT / path,
        ]
        for check in checks:
            if check.exists():
                return check.resolve()
    return (ROOT / "results").resolve()


def normalize_item_name(name: str) -> str:
    return re.sub(r"[^A-Za-z0-9]+", "_", name).strip("_")


def display_name_from_stem(stem: str) -> str:
    replacements = {
        "shadcn_ui": "shadcn/ui",
        "gpui_component": "gpui-component",
        "Zed_UI_GPUI": "Zed UI / GPUI",
        "Cargo_crates_io_cargo_generate_xtask_scaffold": "Cargo / crates.io / cargo-generate / xtask scaffold",
        "Design_Tokens_Community_Group_Style_Dictionary": "Design Tokens Community Group / Style Dictionary",
        "TanStack_Table_TanStack_Virtual": "TanStack Table / TanStack Virtual",
        "React_Aria_React_Aria_Components": "React Aria / React Aria Components",
    }
    return replacements.get(stem, stem.replace("_", " "))


def category_aliases(category_name: str) -> list[str]:
    base = CATEGORY_MAPPING.get(category_name, [])
    slug = normalize_item_name(category_name)
    return list(dict.fromkeys([category_name, slug, slug.lower(), *base]))


def parse_field_categories(fields_config: dict[str, Any]) -> list[dict[str, Any]]:
    raw_categories = fields_config.get("field_categories") or fields_config.get("field_groups") or []
    categories: list[dict[str, Any]] = []

    if isinstance(raw_categories, dict):
        raw_iterable = [
            {"category": category_name, "fields": fields}
            for category_name, fields in raw_categories.items()
        ]
    else:
        raw_iterable = raw_categories

    for raw_category in raw_iterable:
        if isinstance(raw_category, str):
            categories.append({"category": raw_category, "fields": []})
            continue
        if not isinstance(raw_category, dict):
            continue

        category_name = (
            raw_category.get("category")
            or raw_category.get("name")
            or raw_category.get("title")
            or "未分类"
        )
        raw_fields = raw_category.get("fields") or []
        if isinstance(raw_fields, dict):
            field_entries = [
                {"name": field_name, **(meta if isinstance(meta, dict) else {})}
                for field_name, meta in raw_fields.items()
            ]
        else:
            field_entries = raw_fields

        fields: list[dict[str, Any]] = []
        for raw_field in field_entries:
            if isinstance(raw_field, str):
                fields.append({"name": raw_field})
            elif isinstance(raw_field, dict) and raw_field.get("name"):
                fields.append(raw_field)
        categories.append({"category": str(category_name), "fields": fields})

    return categories


def iter_nested_dicts(value: Any) -> list[dict[str, Any]]:
    dicts: list[dict[str, Any]] = []
    if isinstance(value, dict):
        dicts.append(value)
        for nested_value in value.values():
            dicts.extend(iter_nested_dicts(nested_value))
    elif isinstance(value, list):
        for item in value:
            dicts.extend(iter_nested_dicts(item))
    return dicts


def find_field(data: dict[str, Any], field_name: str, category_name: str) -> Any:
    if field_name in data:
        return data[field_name]

    for alias in category_aliases(category_name):
        nested = data.get(alias)
        if isinstance(nested, dict) and field_name in nested:
            return nested[field_name]

    for nested in iter_nested_dicts(data):
        if field_name in nested:
            return nested[field_name]

    return None


def contains_uncertain_marker(value: Any) -> bool:
    if isinstance(value, str):
        return "[不确定]" in value
    if isinstance(value, dict):
        return any(contains_uncertain_marker(nested) for nested in value.values())
    if isinstance(value, list):
        return any(contains_uncertain_marker(item) for item in value)
    return False


def should_skip(field_name: str, value: Any, uncertain_fields: set[str]) -> bool:
    if field_name in uncertain_fields:
        return True
    if value is None:
        return True
    if isinstance(value, str) and not value.strip():
        return True
    if isinstance(value, (list, dict)) and not value:
        return True
    return contains_uncertain_marker(value)


def plain_text(value: Any) -> str:
    if isinstance(value, str):
        return re.sub(r"\s+", " ", value).strip()
    if isinstance(value, list):
        return ", ".join(plain_text(item) for item in value if plain_text(item))
    if isinstance(value, dict):
        parts = []
        for key, nested in value.items():
            text = plain_text(nested)
            if text:
                parts.append(f"{key}: {text}")
        return "; ".join(parts)
    if value is None:
        return ""
    return str(value)


def truncate(value: Any, limit: int = 96) -> str:
    text = plain_text(value)
    if len(text) <= limit:
        return text
    return f"{text[: limit - 1].rstrip()}..."


def wrap_blockquote(text: str, width: int = 120) -> str:
    paragraphs = [paragraph.strip() for paragraph in text.splitlines() if paragraph.strip()]
    if not paragraphs:
        paragraphs = [text.strip()]
    lines: list[str] = []
    for paragraph in paragraphs:
        wrapped = textwrap.wrap(
            paragraph,
            width=width,
            break_long_words=False,
            replace_whitespace=False,
        ) or [paragraph]
        lines.extend(f"> {line}" for line in wrapped)
        lines.append(">")
    while lines and lines[-1] == ">":
        lines.pop()
    return "\n".join(lines)


def format_value(value: Any, depth: int = 0) -> str:
    if isinstance(value, str):
        value = value.strip()
        if len(value) > 100:
            return wrap_blockquote(value)
        return value

    if isinstance(value, dict):
        lines: list[str] = []
        for key, nested in value.items():
            rendered = format_value(nested, depth + 1)
            if "\n" in rendered:
                lines.append(f"- **{key}**:\n{indent(rendered, 2)}")
            else:
                lines.append(f"- **{key}**: {rendered}")
        return "\n".join(lines)

    if isinstance(value, list):
        if not value:
            return ""
        if all(isinstance(item, dict) for item in value):
            lines = []
            for item in value:
                parts = [f"{key}: {plain_text(nested)}" for key, nested in item.items()]
                lines.append(f"- {' | '.join(parts)}")
            return "\n".join(lines)
        if all(not isinstance(item, (dict, list)) for item in value):
            joined = ", ".join(plain_text(item) for item in value)
            if len(joined) <= 120:
                return joined
        return "\n".join(f"- {format_value(item, depth + 1)}" for item in value)

    return str(value)


def indent(text: str, spaces: int) -> str:
    prefix = " " * spaces
    return "\n".join(f"{prefix}{line}" if line else line for line in text.splitlines())


def slugify(value: str, fallback: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return slug or fallback


def collect_known_category_keys(categories: list[dict[str, Any]]) -> set[str]:
    keys = set(CATEGORY_MAPPING)
    for aliases in CATEGORY_MAPPING.values():
        keys.update(aliases)
    for category in categories:
        keys.update(category_aliases(category["category"]))
    return keys


def collect_extra_fields(
    value: Any,
    defined_fields: set[str],
    uncertain_fields: set[str],
    known_category_keys: set[str],
    prefix: str = "",
) -> dict[str, Any]:
    extras: dict[str, Any] = {}
    if not isinstance(value, dict):
        return extras

    for key, nested in value.items():
        if key in {"_source_file", "uncertain"}:
            continue

        if prefix == "" and key in known_category_keys and isinstance(nested, dict):
            extras.update(
                collect_extra_fields(
                    nested,
                    defined_fields,
                    uncertain_fields,
                    known_category_keys,
                    "",
                )
            )
            continue

        path = f"{prefix}.{key}" if prefix else key
        leaf_name = key
        if leaf_name in defined_fields or should_skip(leaf_name, nested, uncertain_fields):
            continue

        if isinstance(nested, dict):
            nested_extras = collect_extra_fields(
                nested,
                defined_fields,
                uncertain_fields,
                known_category_keys,
                path,
            )
            extras.update(nested_extras)
        else:
            extras[path] = nested

    return extras


def ordered_result_files(results_dir: Path, outline: dict[str, Any]) -> list[Path]:
    available = {path.stem: path for path in results_dir.glob("*.json")}
    ordered: list[Path] = []

    for item in outline.get("items") or []:
        if not isinstance(item, dict):
            continue
        stem = normalize_item_name(str(item.get("name", "")))
        path = available.pop(stem, None)
        if path:
            ordered.append(path)

    ordered.extend(path for _, path in sorted(available.items()))
    return ordered


def outline_metadata(outline: dict[str, Any]) -> dict[str, dict[str, Any]]:
    metadata: dict[str, dict[str, Any]] = {}
    for item in outline.get("items") or []:
        if not isinstance(item, dict):
            continue
        metadata[normalize_item_name(str(item.get("name", "")))] = item
    return metadata


def main() -> None:
    outline = load_yaml(ROOT / "outline.yaml")
    fields_config = load_yaml(ROOT / "fields.yaml")
    categories = parse_field_categories(fields_config)
    results_dir = resolve_results_dir(outline)
    result_files = ordered_result_files(results_dir, outline)
    metadata_by_stem = outline_metadata(outline)

    defined_fields = {
        field["name"]
        for category in categories
        for field in category.get("fields", [])
        if field.get("name")
    }
    known_category_keys = collect_known_category_keys(categories)

    report_lines: list[str] = [
        f"# {outline.get('topic') or ROOT.name} - 调研汇总",
        "",
        f"- 生成时间：{datetime.now().strftime('%Y-%m-%d %H:%M:%S')}",
        f"- 结果目录：`{results_dir}`",
        f"- 样本数量：{len(result_files)}",
        "",
        "## 目录",
        "",
    ]

    loaded_results: list[tuple[Path, dict[str, Any], dict[str, Any], str, str]] = []
    used_anchors: set[str] = set()

    for index, path in enumerate(result_files, start=1):
        data = load_json(path)
        metadata = metadata_by_stem.get(path.stem, {})
        name = str(metadata.get("name") or data.get("name") or display_name_from_stem(path.stem))
        anchor = slugify(name, f"item-{index}")
        if anchor in used_anchors:
            anchor = f"{anchor}-{index}"
        used_anchors.add(anchor)
        loaded_results.append((path, data, metadata, name, anchor))

        uncertain_fields = set(data.get("uncertain") or [])
        summary_parts = []
        for field_name in SUMMARY_FIELDS:
            value = find_field(data, field_name, "")
            if should_skip(field_name, value, uncertain_fields):
                continue
            summary_parts.append(f"{field_name}: {truncate(value)}")
        suffix = f" - {' | '.join(summary_parts)}" if summary_parts else ""
        report_lines.append(f"{index}. [{name}](#{anchor}){suffix}")

    report_lines.extend(["", "## 详细内容", ""])

    for index, (path, data, metadata, name, anchor) in enumerate(loaded_results, start=1):
        uncertain_fields = set(data.get("uncertain") or [])
        report_lines.extend(
            [
                f"## <a id=\"{anchor}\"></a>{index}. {name}",
                "",
                f"- 结果文件：`{path.name}`",
            ]
        )

        if metadata.get("category"):
            report_lines.append(f"- 调研类别：`{metadata['category']}`")
        if metadata.get("why"):
            report_lines.extend(["- 纳入原因：", indent(format_value(str(metadata["why"])), 2)])
        if metadata.get("sources"):
            report_lines.append("- 参考来源：")
            for source in metadata["sources"]:
                report_lines.append(f"  - {source}")
        report_lines.append("")

        for category in categories:
            category_name = category["category"]
            category_lines: list[str] = []
            for field in category.get("fields", []):
                field_name = field["name"]
                value = find_field(data, field_name, category_name)
                if should_skip(field_name, value, uncertain_fields):
                    continue
                rendered = format_value(value)
                if not rendered:
                    continue
                category_lines.extend([f"#### `{field_name}`", "", rendered, ""])

            if category_lines:
                report_lines.extend([f"### {category_name}", "", *category_lines])

        extras = collect_extra_fields(
            data,
            defined_fields,
            uncertain_fields,
            known_category_keys,
        )
        if extras:
            report_lines.extend(["### 其他信息", ""])
            for field_name, value in sorted(extras.items()):
                rendered = format_value(value)
                if rendered:
                    report_lines.extend([f"#### `{field_name}`", "", rendered, ""])

        if uncertain_fields:
            report_lines.extend(["### 不确定字段（已跳过）", ""])
            for field_name in sorted(uncertain_fields):
                report_lines.append(f"- `{field_name}`")
            report_lines.append("")

    REPORT_PATH.write_text("\n".join(report_lines).rstrip() + "\n", encoding="utf-8")
    print(f"Wrote {REPORT_PATH}")
    print(f"Items: {len(result_files)}")


if __name__ == "__main__":
    main()

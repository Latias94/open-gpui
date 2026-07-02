use criterion::{Criterion, criterion_group, criterion_main};
use open_gpui::{FontFallbacks, FontRun, PlatformTextSystem, font, px};
use open_gpui_wgpu::CosmicTextSystem;
use std::borrow::Cow;
use std::fs;

const LILEX_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/lilex/Lilex-Regular.ttf"
);
const IBM_PLEX_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"
);

// ~4 000 chars of typical ASCII code text.
fn code_text() -> String {
    concat!(
        "    fn compute_run_spans(\n",
        "        text: &str,\n",
        "        run_offset: usize,\n",
        "        run_len: usize,\n",
        "        primary: FontId,\n",
        "        fallback_chain: &[(FontId, SharedString)],\n",
        "        covers: &impl Fn(FontId, char) -> bool,\n",
        "    ) -> SmallVec<[RunSpan; 4]> {\n",
        "        let mut spans = SmallVec::new();\n",
        "        let run_end = run_offset + run_len;\n",
        "        if run_end <= run_offset { return spans; }\n",
        "        let run_text = &text[run_offset..run_end];\n",
        "        let mut span_start = run_offset;\n",
        "        let mut span_slot: Option<usize> = None;\n",
        "        for (ch_idx, ch) in run_text.char_indices() {\n",
        "            let abs = run_offset + ch_idx;\n",
        "            let next = pick_covering_slot(ch, span_slot, primary, fallback_chain, covers);\n",
        "            if next == span_slot { continue; }\n",
        "            if abs > span_start {\n",
        "                spans.push(RunSpan { start: span_start, end: abs, slot: span_slot });\n",
        "            }\n",
        "            span_start = abs;\n",
        "            span_slot = next;\n",
        "        }\n",
        "        spans\n",
        "    }\n",
    )
    .repeat(8) // ~3 800 chars
}

fn bench_layout_line(c: &mut Criterion) {
    let Some(lilex) = read_font("OPEN_GPUI_LAYOUT_LINE_LILEX", LILEX_PATH) else {
        return;
    };
    let Some(ibm_plex) = read_font("OPEN_GPUI_LAYOUT_LINE_IBM_PLEX", IBM_PLEX_PATH) else {
        return;
    };

    let system = CosmicTextSystem::new_without_system_fonts("Lilex");
    system
        .add_fonts(vec![Cow::Owned(lilex), Cow::Owned(ibm_plex)])
        .unwrap();

    let font_id_no_fallback = system.font_id(&font("Lilex")).unwrap();

    let font_id_with_fallback = {
        let mut f = font("Lilex");
        f.fallbacks = Some(FontFallbacks::from_fonts(vec!["IBM Plex Sans".to_string()]));
        system.font_id(&f).unwrap()
    };

    let text = code_text();

    let runs_no_fallback = vec![FontRun {
        len: text.len(),
        font_id: font_id_no_fallback,
    }];
    let runs_with_fallback = vec![FontRun {
        len: text.len(),
        font_id: font_id_with_fallback,
    }];

    let mut group = c.benchmark_group("layout_line");

    group.bench_function("no_fallback", |b| {
        b.iter(|| system.layout_line(&text, px(14.0), &runs_no_fallback))
    });

    group.bench_function("with_fallback_ascii", |b| {
        b.iter(|| system.layout_line(&text, px(14.0), &runs_with_fallback))
    });

    group.finish();
}

fn read_font(env_var: &str, default_path: &str) -> Option<Vec<u8>> {
    let path = std::env::var(env_var).unwrap_or_else(|_| default_path.to_string());
    match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            eprintln!("skipping layout_line bench: could not read {path}: {err}");
            None
        }
    }
}

criterion_group!(benches, bench_layout_line);
criterion_main!(benches);

use super::bench;
use proc_macro2::TokenStream;
use quote::quote;

fn expansion(args: TokenStream, function: TokenStream) -> String {
    let expanded = bench(args, function);
    syn::parse2::<syn::File>(expanded.clone())
        .expect("benchmark expansion must be valid Rust syntax");
    let expanded = expanded.to_string();
    assert!(!expanded.contains("compile_error"), "{expanded}");
    expanded
}

fn assert_error(args: TokenStream, function: TokenStream, expected: &str) {
    let error = syn::parse2::<syn::ItemMacro>(bench(args, function))
        .expect("invalid benchmark must expand to a compile error");
    assert_eq!(
        error.mac.path.segments.last().unwrap().ident,
        "compile_error"
    );
    let message = syn::parse2::<syn::LitStr>(error.mac.tokens)
        .unwrap()
        .value();
    assert!(
        message.contains(expected),
        "expected {expected:?}, got {message:?}"
    );
}

#[test]
fn benchmark_wrapper_preserves_public_and_restricted_visibility() {
    for visibility in [quote!(pub), quote!(pub(crate)), quote!(pub(super))] {
        let expanded = bench(
            quote!(),
            quote!(#visibility fn refresh(cx: &mut BenchAppContext) {}),
        );
        let file = syn::parse2::<syn::File>(expanded).unwrap();
        let wrapper = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "refresh" => Some(function),
                _ => None,
            })
            .unwrap();
        let actual = &wrapper.vis;
        assert_eq!(visibility.to_string(), quote!(#actual).to_string());
    }
}

#[test]
fn unseeded_benchmark_uses_measurement_and_per_routine_context() {
    let expanded = expansion(
        quote!(),
        quote!(
            fn refresh(cx: &mut BenchAppContext) {}
        ),
    );

    assert!(
        expanded.contains(
            &quote!(fn refresh(criterion: &mut criterion::Criterion<open_gpui::BenchMeasurement>))
                .to_string()
        )
    );
    assert!(
        expanded.contains(&quote!(let report = open_gpui::BenchReport::default();).to_string())
    );
    assert!(
        expanded.contains(&quote!(open_gpui_platform::current_headless_renderer()).to_string())
    );
    assert!(expanded.contains(&quote!(open_gpui_platform::bench_text_system()).to_string()));
    assert!(expanded.contains(&quote!(report.print(&name);).to_string()));
    assert!(!expanded.contains("calculate_seeds"));

    let routine = expanded.find(&quote!(move |bencher|).to_string()).unwrap();
    let context = expanded
        .find(&quote!(open_gpui::BenchAppContext::new_with_platform_and_report).to_string())
        .unwrap();
    let call = expanded
        .find(&quote!(__gpui_bench_refresh(&mut cx);).to_string())
        .unwrap();
    let teardown = expanded.find(&quote!(cx.teardown();).to_string()).unwrap();
    assert!(routine < context && context < call && call < teardown);
}

#[test]
fn by_value_rng_can_precede_or_follow_the_context() {
    for (parameters, call) in [
        (
            quote!(rng: StdRng, cx: &mut BenchAppContext),
            quote!(rng, &mut cx),
        ),
        (
            quote!(cx: &mut BenchAppContext, rng: rand::rngs::StdRng),
            quote!(&mut cx, rng),
        ),
    ] {
        let expanded = expansion(quote!(seed = 7), quote!(fn refresh(#parameters) {}));
        assert!(expanded.contains(&quote!(open_gpui::calculate_seeds(1u64, &[7u64])).to_string()));
        assert!(expanded.contains(&quote!(__gpui_bench_refresh(#call);).to_string()));
        assert!(
            expanded
                .contains(&quote!(format!("{}/seed-{}", stringify!(refresh), seed)).to_string())
        );
        let routine = expanded.find(&quote!(move |bencher|).to_string()).unwrap();
        let rng = expanded
            .find(&quote!(open_gpui::private::rand::SeedableRng::seed_from_u64(seed)).to_string())
            .unwrap();
        assert!(routine < rng, "each routine must restart the same seed");
    }
}

#[test]
fn parameterized_seeded_benchmark_preserves_rng_positions_and_separate_reports() {
    for (parameters, call) in [
        (
            quote!(rng: StdRng, input: &usize, cx: &mut BenchAppContext),
            quote!(rng, input, &mut cx),
        ),
        (
            quote!(input: &usize, rng: StdRng, cx: &mut BenchAppContext),
            quote!(input, rng, &mut cx),
        ),
        (
            quote!(input: &usize, cx: &mut BenchAppContext, rng: StdRng),
            quote!(input, &mut cx, rng),
        ),
    ] {
        let expanded = expansion(
            quote!(
                inputs = [10, 20],
                input_name = "size",
                group = "refresh",
                sample_size = 25,
                fps = 120,
                seeds(9, 13,),
                iterations = 2,
            ),
            quote!(fn refresh(#parameters) {}),
        );
        assert!(expanded.contains(&quote!(__gpui_bench_refresh(#call);).to_string()));
        assert!(expanded.contains(&quote!(criterion.benchmark_group("refresh")).to_string()));
        assert!(expanded.contains(&quote!(group.sample_size(25usize);).to_string()));
        assert!(
            expanded
                .contains(&quote!(open_gpui::calculate_seeds(2u64, &[9u64, 13u64])).to_string())
        );
        assert!(expanded.contains(&quote!(format!("{}/seed-{}", input, seed)).to_string()));
        assert!(
            expanded
                .contains(&quote!(format!("{}/{}/{}", "refresh", "size", parameter)).to_string())
        );
        assert!(
            expanded.contains(&quote!(criterion::BenchmarkId::new("size", &parameter)).to_string())
        );
        assert!(expanded.contains(&quote!(report.print(&report_name);).to_string()));
        assert!(expanded.contains(&quote!(group.finish();).to_string()));
        let input = expanded.find("for input").unwrap();
        let seed = expanded.find("for seed").unwrap();
        let report = expanded
            .find(&quote!(let report = open_gpui::BenchReport::with_fps(120u64);).to_string())
            .unwrap();
        let routine = expanded
            .find(&quote!(move |bencher, input|).to_string())
            .unwrap();
        assert!(input < seed && seed < report && report < routine);
    }
}

#[test]
fn unseeded_inputs_default_to_the_function_name() {
    let expanded = expansion(
        quote!(inputs = sizes()),
        quote!(
            fn refresh(input: &usize, cx: &mut BenchAppContext) {}
        ),
    );
    assert!(expanded.contains(&quote!(criterion.benchmark_group(stringify!(refresh))).to_string()));
    assert!(expanded.contains(
        &quote!(criterion::BenchmarkId::new(stringify!(refresh), &parameter)).to_string()
    ));
    assert!(expanded.contains(&quote!(let parameter = input.to_string();).to_string()));
    assert!(!expanded.contains("calculate_seeds"));
    assert!(!expanded.contains("seed_from_u64"));
}

#[test]
fn seeds_accept_empty_lists_and_full_width_integer_literals() {
    let expanded = expansion(
        quote!(seeds()),
        quote!(
            fn refresh(rng: StdRng, cx: &mut BenchAppContext) {}
        ),
    );
    assert!(expanded.contains(&quote!(open_gpui::calculate_seeds(1u64, &[])).to_string()));
    let expanded = expansion(
        quote!(seeds(0, 0xff, 18_446_744_073_709_551_615)),
        quote!(
            fn refresh(rng: StdRng, cx: &mut BenchAppContext) {}
        ),
    );
    assert!(
        expanded.contains(
            &quote!(open_gpui::calculate_seeds(
                1u64,
                &[0u64, 255u64, 18446744073709551615u64]
            ))
            .to_string()
        )
    );
}

#[test]
fn invalid_options_emit_macro_diagnostics() {
    for (args, expected) in [
        (quote!(fps = 0), "`fps` must be greater than zero"),
        (
            quote!(sample_size = 0),
            "`sample_size` must be greater than zero",
        ),
        (
            quote!(iterations = 0),
            "`iterations` must be greater than zero",
        ),
        (
            quote!(input_name = "size"),
            "`input_name` requires `inputs`",
        ),
        (quote!(group = "refresh"), "`group` requires `inputs`"),
        (quote!(sample_size = 20), "`sample_size` requires `inputs`"),
        (quote!(retries = 1), "only accepts"),
        (quote!(fps = "fast"), "expected integer literal"),
        (
            quote!(inputs = [1], input_name = 3),
            "expected string literal",
        ),
    ] {
        assert_error(
            args,
            quote!(
                fn refresh(cx: &mut BenchAppContext) {}
            ),
            expected,
        );
    }
}

#[test]
fn seed_options_require_a_rng_and_reject_duplicate_seed_declarations() {
    for args in [quote!(seed = 7), quote!(seeds()), quote!(iterations = 2)] {
        assert_error(
            args,
            quote!(
                fn refresh(cx: &mut BenchAppContext) {}
            ),
            "`seed`, `seeds`, and `iterations` require a `StdRng` parameter",
        );
    }
    for args in [
        quote!(seed = 7, seeds(8)),
        quote!(seeds(8), seed = 7),
        quote!(seed = 7, seed = 8),
        quote!(seeds(), seeds(8)),
    ] {
        assert_error(
            args,
            quote!(
                fn refresh(rng: StdRng, cx: &mut BenchAppContext) {}
            ),
            "accepts one of `seed = N` or `seeds(...)`, once",
        );
    }
}

#[test]
fn rng_references_and_multiple_rngs_are_rejected() {
    for parameters in [
        quote!(rng: &StdRng, cx: &mut BenchAppContext),
        quote!(cx: &mut BenchAppContext, rng: &mut rand::rngs::StdRng),
    ] {
        assert_error(
            quote!(),
            quote!(fn refresh(#parameters) {}),
            "provides `StdRng` by value; take `rng: StdRng`",
        );
    }
    assert_error(
        quote!(),
        quote!(
            fn refresh(a: StdRng, cx: &mut BenchAppContext, b: StdRng) {}
        ),
        "provides one `StdRng`",
    );
}

#[test]
fn invalid_signatures_and_the_old_bencher_parameter_are_rejected() {
    for function in [
        quote!(
            fn refresh() {}
        ),
        quote!(
            fn refresh(rng: StdRng) {}
        ),
        quote!(
            fn refresh(bencher: &mut criterion::Bencher, cx: &mut BenchAppContext) {}
        ),
    ] {
        assert_error(quote!(), function, "expected `(cx: &mut BenchAppContext)`");
    }
    assert_error(
        quote!(inputs = [1]),
        quote!(
            fn refresh(cx: &mut BenchAppContext) {}
        ),
        "expected `(input: &Input, cx: &mut BenchAppContext)`",
    );
    assert_error(
        quote!(),
        quote!(
            async fn refresh(cx: &mut BenchAppContext) {}
        ),
        "does not support async benchmark functions",
    );
    assert_error(
        quote!(),
        quote!(
            fn refresh(&self) {}
        ),
        "functions cannot take `self`",
    );
}

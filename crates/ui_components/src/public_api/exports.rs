//! Declarative public exports that emit their own typed conformance facts.

macro_rules! declare_public_exports {
    (
        common $constant:ident;
        $(
            $module:path => { $($name:ident),+ $(,)? }
        ),+ $(,)?
    ) => {
        $crate::public_api::declare_public_exports!(@plain_exports;
            $( $module => { $($name),+ } ),+
        );
        $crate::public_api::declare_public_exports!(@facts
            $constant,
            "open_gpui_ui_components::{root,common,prelude}",
            $crate::component_contract::PublicApiTier::Common;
            $( $module => { $($name),+ } ),+
        );
        $crate::public_api::declare_public_exports!(@common_witness;
            $( $module => { $($name),+ } ),+
        );
    };
    (
        extended $constant:ident;
        $(
            $module:path => { $($name:ident),+ $(,)? }
        ),+ $(,)?
    ) => {
        $crate::public_api::declare_public_exports!(@plain_exports;
            $( $module => { $($name),+ } ),+
        );
        $crate::public_api::declare_public_exports!(@facts
            $constant,
            "open_gpui_ui_components::root",
            $crate::component_contract::PublicApiTier::Extended;
            $( $module => { $($name),+ } ),+
        );
        $crate::public_api::declare_public_exports!(@root_witness;
            $( $module => { $($name),+ } ),+
        );
    };
    (
        diagnostic $constant:ident;
        $(
            $module:path => { $($name:ident),+ $(,)? }
        ),+ $(,)?
    ) => {
        $crate::public_api::declare_public_exports!(@diagnostic_exports;
            $( $module => { $($name),+ } ),+
        );
        $crate::public_api::declare_public_exports!(@facts
            $constant,
            "open_gpui_ui_components::table",
            $crate::component_contract::PublicApiTier::Diagnostic;
            $( $module => { $($name),+ } ),+
        );
        $crate::public_api::declare_public_exports!(@table_witness;
            $( $module => { $($name),+ } ),+
        );
    };
    (@plain_exports;
        $( $module:path => { $($name:ident),+ } ),+
    ) => {
        $(
            pub use $module::{ $($name),+ };
        )+
    };
    (@diagnostic_exports;
        $( $module:path => { $($name:ident),+ } ),+
    ) => {
        $(
            $(
                #[doc = concat!(
                    "This diagnostic remains available only through its explicit owner module.\n\n",
                    "```compile_fail,E0432\n",
                    "use open_gpui_ui_components::", stringify!($name), ";\n",
                    "```\n\n",
                    "```compile_fail,E0432\n",
                    "use open_gpui_ui_components::common::", stringify!($name), ";\n",
                    "```\n\n",
                    "```compile_fail,E0432\n",
                    "use open_gpui_ui_components::prelude::", stringify!($name), ";\n",
                    "```",
                )]
                pub use $module::{ $name };
            )+
        )+
    };
    (@facts
        $constant:ident,
        $owner:literal,
        $tier:expr;
        $( $module:path => { $($name:ident),+ } ),+
    ) => {
        pub(crate) const $constant: &[$crate::component_contract::PublicApiExport] = &[
            $(
                $(
                    $crate::component_contract::PublicApiExport::new(
                        stringify!($name),
                        $owner,
                        $tier,
                    ),
                )+
            )+
        ];
    };
    (@common_witness;
        $( $module:path => { $($name:ident),+ } ),+
    ) => {
        $(
            $(
                const _: () = {
                    #[allow(unused_imports)]
                    use $crate::$name as _;
                    #[allow(unused_imports)]
                    use $crate::common::$name as _;
                    #[allow(unused_imports)]
                    use $crate::prelude::$name as _;
                };
            )+
        )+
    };
    (@root_witness;
        $( $module:path => { $($name:ident),+ } ),+
    ) => {
        $(
            $(
                const _: () = {
                    #[allow(unused_imports)]
                    use $crate::$name as _;
                };
            )+
        )+
    };
    (@table_witness;
        $( $module:path => { $($name:ident),+ } ),+
    ) => {
        $(
            $(
                const _: () = {
                    #[allow(unused_imports)]
                    use $crate::table::$name as _;
                };
            )+
        )+
    };
}

pub(crate) use declare_public_exports;

use jellyflow_open_gpui::testing::{
    OpenGpuiProductFixtureCase, OpenGpuiProductFixtureFamily, product_fixture_catalog,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductGalleryCase {
    pub(crate) fixture: OpenGpuiProductFixtureCase,
    pub(crate) label: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) accent: u32,
}

impl ProductGalleryCase {
    pub(crate) fn id(&self) -> &str {
        self.fixture.id.as_str()
    }

    pub(crate) fn kit_key(&self) -> &str {
        self.fixture.kit_key.as_str()
    }

    pub(crate) fn fixture_key(&self) -> &str {
        self.fixture.fixture_key.as_str()
    }

    pub(crate) fn family_label(&self) -> &'static str {
        match self.fixture.family {
            OpenGpuiProductFixtureFamily::Workflow => "Dify workflow",
            OpenGpuiProductFixtureFamily::ShaderGraph => "Shader graph",
            OpenGpuiProductFixtureFamily::Erd => "ERD",
            OpenGpuiProductFixtureFamily::MindMap => "Mind map",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductGallerySelection {
    active_id: String,
}

impl ProductGallerySelection {
    pub(crate) fn new(active_id: impl Into<String>) -> Self {
        Self {
            active_id: active_id.into(),
        }
    }

    pub(crate) fn active_id(&self) -> &str {
        self.active_id.as_str()
    }

    pub(crate) fn set_active(&mut self, id: impl Into<String>) {
        self.active_id = id.into();
    }

    pub(crate) fn active_case<'a>(
        &self,
        cases: &'a [ProductGalleryCase],
    ) -> &'a ProductGalleryCase {
        cases
            .iter()
            .find(|case| case.id() == self.active_id)
            .unwrap_or_else(|| {
                cases
                    .first()
                    .expect("product gallery must contain at least one fixture")
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductGalleryState {
    cases: Vec<ProductGalleryCase>,
    selection: ProductGallerySelection,
}

impl ProductGalleryState {
    pub(crate) fn new(cases: Vec<ProductGalleryCase>, active_id: impl Into<String>) -> Self {
        assert!(
            !cases.is_empty(),
            "product gallery state requires at least one fixture"
        );
        Self {
            cases,
            selection: ProductGallerySelection::new(active_id),
        }
    }

    pub(crate) fn default() -> Self {
        let cases = product_gallery_cases();
        let active_id = cases
            .first()
            .expect("product gallery catalog should not be empty")
            .id()
            .to_owned();
        Self::new(cases, active_id)
    }

    pub(crate) fn cases(&self) -> &[ProductGalleryCase] {
        self.cases.as_slice()
    }

    pub(crate) fn active_id(&self) -> &str {
        self.selection.active_id()
    }

    pub(crate) fn active_case(&self) -> &ProductGalleryCase {
        self.selection.active_case(&self.cases)
    }

    pub(crate) fn set_active(&mut self, id: impl Into<String>) {
        self.selection.set_active(id);
    }
}

pub(crate) fn product_gallery_cases() -> Vec<ProductGalleryCase> {
    product_fixture_catalog()
        .into_iter()
        .map(|fixture| {
            let (label, summary, accent) = match fixture.id.as_str() {
                "workflow.review" => (
                    "Workflow review",
                    "Dify-style automation cards with prompts, actions, and inspector paths.",
                    0x0f766e,
                ),
                "shader.material_mix" => (
                    "Material mix",
                    "Shader graph cards with typed rails, dynamic inputs, and missing-port diagnostics.",
                    0x7c3aed,
                ),
                "erd.customer_orders" => (
                    "Customer orders",
                    "Database table cards with field rows, relation handles, and explicit port downgrades.",
                    0x2563eb,
                ),
                "mind-map.strategy" => (
                    "Strategy map",
                    "MarginNote-style topic and source cards with shell-friendly previews.",
                    0x0891b2,
                ),
                _ => (
                    "Product fixture",
                    "Jellyflow product-shaped Open GPUI gallery fixture.",
                    0x475569,
                ),
            };
            ProductGalleryCase {
                fixture,
                label,
                summary,
                accent,
            }
        })
        .collect()
}

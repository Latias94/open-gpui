use crate::{
    DockActionApplyError, DockItemId, DockPanelCatalog, DockPanelDescriptor, DockWorkspace,
};

pub(crate) struct DockWorkspacePanelLifecycle<'a> {
    catalog: &'a DockPanelCatalog,
}

impl<'a> DockWorkspacePanelLifecycle<'a> {
    pub(crate) fn new(catalog: &'a DockPanelCatalog) -> Self {
        Self { catalog }
    }

    pub(crate) fn validate_open(&self, item: &DockItemId) -> Result<(), DockActionApplyError> {
        self.require_descriptor(item)?;
        Ok(())
    }

    pub(crate) fn validate_close(&self, item: &DockItemId) -> Result<(), DockActionApplyError> {
        let descriptor = self.require_descriptor(item)?;
        if descriptor.is_closable() {
            Ok(())
        } else {
            Err(DockActionApplyError::PanelNotClosable { item: item.clone() })
        }
    }

    fn require_descriptor(
        &self,
        item: &DockItemId,
    ) -> Result<&'a DockPanelDescriptor, DockActionApplyError> {
        self.catalog
            .descriptor(item)
            .ok_or_else(|| DockActionApplyError::PanelNotRegistered { item: item.clone() })
    }
}

impl DockWorkspace {
    pub(crate) fn panel_lifecycle(&self) -> DockWorkspacePanelLifecycle<'_> {
        DockWorkspacePanelLifecycle::new(self.panels().catalog())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DockPanelDescriptor;

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    #[test]
    fn open_requires_descriptor_metadata() {
        let mut catalog = DockPanelCatalog::default();
        let lifecycle = DockWorkspacePanelLifecycle::new(&catalog);

        assert_eq!(
            lifecycle.validate_open(&item("missing")),
            Err(DockActionApplyError::PanelNotRegistered {
                item: item("missing")
            })
        );

        catalog.register(item("restored"), DockPanelDescriptor::new("Restored"));
        let lifecycle = DockWorkspacePanelLifecycle::new(&catalog);

        assert_eq!(lifecycle.validate_open(&item("restored")), Ok(()));
    }

    #[test]
    fn close_respects_descriptor_close_policy() {
        let mut catalog = DockPanelCatalog::default();
        catalog.register(
            item("locked"),
            DockPanelDescriptor::new("Locked").closable(false),
        );
        catalog.register(item("closable"), DockPanelDescriptor::new("Closable"));

        let lifecycle = DockWorkspacePanelLifecycle::new(&catalog);

        assert_eq!(
            lifecycle.validate_close(&item("locked")),
            Err(DockActionApplyError::PanelNotClosable {
                item: item("locked")
            })
        );
        assert_eq!(lifecycle.validate_close(&item("closable")), Ok(()));
    }
}

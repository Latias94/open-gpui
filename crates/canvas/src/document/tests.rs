use super::*;
use crate::test_support::{
    CanvasCommandGenerator, TestRng, connected_pair_fixture, document_fixture,
};
use crate::{
    CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION, CANVAS_SNAPSHOT_MIGRATIONS,
    CanvasRecordParentRelation,
};
use open_gpui::{point, px, size};

mod builder;
mod edge_validation;
mod relations;
mod snapshot;
mod transactions;

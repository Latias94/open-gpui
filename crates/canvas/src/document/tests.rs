use super::*;
use crate::format::{CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION, CANVAS_SNAPSHOT_MIGRATIONS};
use crate::relations::CanvasRecordParentRelation;
use crate::test_support::{
    CanvasCommandGenerator, TestRng, connected_pair_fixture, document_fixture,
};
use open_gpui::{point, px, size};

mod builder;
mod edge_validation;
mod relations;
mod snapshot;
mod transactions;

use super::frame::{CanvasPreparedPaintFrame, CanvasSceneLayerPhase, prepaint_canvas_frame};
use super::input::{CanvasEditorInputHandler, register_canvas_editor_input};
use super::model::{CanvasPaintModel, CanvasPaintOptions, CanvasPaintTheme};
use super::painter::{paint_canvas_frame, paint_canvas_scene_phase};
use open_gpui::{Bounds, Canvas, Context, Entity, FocusHandle, Pixels, Window, canvas};

pub fn canvas_view(
    model: CanvasPaintModel,
    options: CanvasPaintOptions,
    theme: CanvasPaintTheme,
) -> Canvas<CanvasPreparedPaintFrame> {
    let prepaint_model = model.clone();
    canvas(
        move |bounds, window, _| {
            prepaint_canvas_frame(&prepaint_model, bounds, options, theme, window)
        },
        move |bounds, frame, window, cx| {
            paint_canvas_frame(bounds, &model, &frame, theme, window, cx);
        },
    )
}

pub fn canvas_scene_view(
    model: CanvasPaintModel,
    options: CanvasPaintOptions,
    theme: CanvasPaintTheme,
    phases: impl Into<Vec<CanvasSceneLayerPhase>>,
) -> Canvas<CanvasPreparedPaintFrame> {
    let phases = phases.into();
    let prepaint_model = model.clone();
    canvas(
        move |bounds, window, _| {
            prepaint_canvas_frame(&prepaint_model, bounds, options, theme, window)
        },
        move |bounds, frame, window, cx| {
            for phase in &phases {
                paint_canvas_scene_phase(bounds, &model, &frame, *phase, theme, window, cx);
            }
        },
    )
}

pub fn canvas_editor_view<T>(
    model: CanvasPaintModel,
    entity: Entity<T>,
    focus_handle: FocusHandle,
    handler: CanvasEditorInputHandler<T>,
    options: CanvasPaintOptions,
    theme: CanvasPaintTheme,
) -> Canvas<CanvasPreparedPaintFrame>
where
    T: 'static,
{
    let prepaint_model = model.clone();
    canvas(
        move |bounds, window, _| {
            prepaint_canvas_frame(&prepaint_model, bounds, options, theme, window)
        },
        move |bounds, frame, window, cx| {
            register_canvas_editor_input(entity, focus_handle, bounds, handler, window);
            paint_canvas_frame(bounds, &model, &frame, theme, window, cx);
        },
    )
}

pub fn canvas_editor_view_with_frame<T>(
    model: CanvasPaintModel,
    entity: Entity<T>,
    focus_handle: FocusHandle,
    handler: CanvasEditorInputHandler<T>,
    options: CanvasPaintOptions,
    theme: CanvasPaintTheme,
    on_frame: impl Fn(&mut T, &mut Window, Bounds<Pixels>, &CanvasPreparedPaintFrame, &mut Context<T>)
    + 'static,
) -> Canvas<CanvasPreparedPaintFrame>
where
    T: 'static,
{
    let prepaint_model = model.clone();
    canvas(
        move |bounds, window, _| {
            prepaint_canvas_frame(&prepaint_model, bounds, options, theme, window)
        },
        move |bounds, frame, window, cx| {
            register_canvas_editor_input(entity.clone(), focus_handle, bounds, handler, window);
            entity.update(cx, |target, cx| {
                on_frame(target, window, bounds, &frame, cx)
            });
            paint_canvas_frame(bounds, &model, &frame, theme, window, cx);
        },
    )
}

pub fn canvas_editor_scene_view_with_frame<T>(
    model: CanvasPaintModel,
    entity: Entity<T>,
    focus_handle: FocusHandle,
    handler: CanvasEditorInputHandler<T>,
    options: CanvasPaintOptions,
    theme: CanvasPaintTheme,
    phases: impl Into<Vec<CanvasSceneLayerPhase>>,
    on_frame: impl Fn(&mut T, &mut Window, Bounds<Pixels>, &CanvasPreparedPaintFrame, &mut Context<T>)
    + 'static,
) -> Canvas<CanvasPreparedPaintFrame>
where
    T: 'static,
{
    let phases = phases.into();
    let prepaint_model = model.clone();
    canvas(
        move |bounds, window, _| {
            prepaint_canvas_frame(&prepaint_model, bounds, options, theme, window)
        },
        move |bounds, frame, window, cx| {
            register_canvas_editor_input(entity.clone(), focus_handle, bounds, handler, window);
            entity.update(cx, |target, cx| {
                on_frame(target, window, bounds, &frame, cx)
            });
            for phase in &phases {
                paint_canvas_scene_phase(bounds, &model, &frame, *phase, theme, window, cx);
            }
        },
    )
}

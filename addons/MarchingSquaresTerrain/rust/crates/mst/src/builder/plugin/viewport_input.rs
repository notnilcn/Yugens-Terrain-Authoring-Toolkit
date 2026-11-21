use super::*;
impl PixelBuilderPlugin {
    pub(super) fn route_viewport_input(
        &mut self,
        lens: Option<Gd<Camera3D>>,
        happening: Option<Gd<InputEvent>>,
    ) -> i32 {
        let pass = AfterGuiInput::PASS.ord();
        let stop = AfterGuiInput::STOP.ord();
        let (Some(camera), Some(event)) = (lens, happening) else {
            return pass;
        };
        if !self.visible || (self.target.is_none() && self.host.is_none()) {
            return pass;
        }
        if self
            .camera
            .as_ref()
            .is_none_or(|c| c.instance_id() != camera.instance_id())
        {
            if let Some(LayoutUi::Modeler(x)) = &mut self.ui
                && let Some(i) = super::super::modeler_ui::hud::viewport_of(&camera)
            {
                x.hud.set_viewport(i);
            }
        }
        self.camera = Some(camera.clone());
        if self.adjust.is_some()
            && let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>()
            && mb.is_pressed()
            && mb.get_button_index() == MouseButton::LEFT
        {
            self.adjust_commit();
        }
        let editing =
            self.target.is_some() && self.mode != Mode::Object && self.tool == Tool::Select;
        let placing = self.tool != Tool::Select;
        if self.tool == Tool::Knife
            && self.target.is_some()
            && let Some(r) = self.knife_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::Bezier
            && let Some(r) = self.bezier_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::Shape
            && self.spec.sort.is_polyline()
            && let Some(r) = self.wall_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.insert_vert
            && self.mode == Mode::Edge
            && self.target.is_some()
            && self.tool == Tool::Select
            && let Some(r) = self.insert_vert_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
            return if self.on_key(&key) { stop } else { pass };
        }
        if self.tool == Tool::Shape
            && let Some(r) = self.shape_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::Paint
            && let Some(r) = self.paint_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if self.tool == Tool::VertexPaint
            && let Some(r) = self.vpaint_event(&camera, &event)
        {
            return if r { stop } else { pass };
        }
        if let Ok(mm) = event.clone().try_cast::<InputEventMouseMotion>() {
            let pos = mm.get_position();
            if let Some(p) = self.ctx_press {
                self.ctx_travel += mm.get_relative().length();
                if p.distance_to(pos) > context_menu::CLICK_SLOP
                    || self.ctx_travel > context_menu::CLICK_SLOP
                {
                    self.ctx_press = None;
                }
            }
            if self.drag.is_some() {
                self.drag_motion(&camera, pos);
                return stop;
            }
            if let Some(m) = &mut self.marquee {
                m.to = pos;
                self.base().update_overlays();
                return stop;
            }
            if editing {
                let h = self.gizmo_hit(&camera, pos);
                if h != self.hover_handle {
                    self.hover_handle = h;
                    self.base().update_overlays();
                }
            }
            self.update_hover(&camera, pos);
            return pass;
        }
        if let Ok(mb) = event.try_cast::<InputEventMouseButton>() {
            let button = mb.get_button_index();
            let pos = mb.get_position();
            let shift = Input::singleton().is_key_pressed(Key::SHIFT);
            if button == MouseButton::RIGHT
                && mb.is_pressed()
                && (self.drag.is_some() || self.marquee.is_some())
            {
                self.cancel_drag();
                self.marquee = None;
                self.redraw();
                return stop;
            }
            if button == MouseButton::RIGHT
                && !mb.is_shift_pressed()
                && !mb.is_alt_pressed()
                && !mb.is_ctrl_pressed()
                && !mb.is_meta_pressed()
            {
                if mb.is_pressed() {
                    self.ctx_press = None;
                    self.ctx_travel = 0.0;
                    if self.context_menu_available()
                        && self.menu_ctx_addressees_picked(&camera, pos)
                    {
                        self.ctx_press = Some(pos);
                    }
                } else if let Some(p) = self.ctx_press.take() {
                    self.ctx_pending = Some(p);
                    self.to_gd().call_deferred("open_context_menu_pending", &[]);
                }
                return pass;
            }
            if button != MouseButton::LEFT {
                return pass;
            }
            if placing {
                if mb.is_pressed() {
                    self.update_hover(&camera, pos);
                    match self.tool {
                        Tool::Shape => self.stamp_shape(shift),
                        Tool::Place => self.place_target(),
                        Tool::Select
                        | Tool::Paint
                        | Tool::Knife
                        | Tool::VertexPaint
                        | Tool::Bezier => {}
                    }
                }
                return stop;
            }
            if !editing {
                return pass;
            }
            if mb.is_pressed() {
                if mb.is_double_click()
                    && self.drag.is_none()
                    && self.double_click(&camera, pos, shift, mb.is_alt_pressed())
                {
                    self.marquee = None;
                } else if let Some(h) = self.gizmo_hit(&camera, pos) {
                    self.begin_drag(&camera, pos, h, shift);
                } else if !self.choose(&camera, pos, shift) {
                    self.marquee = Some(Marquee {
                        start_pt: pos,
                        to: pos,
                        additive: shift,
                    });
                } else {
                    self.load_face_props();
                }
            } else if self.drag.is_some() {
                self.finish_drag();
            } else if let Some(m) = self.marquee.take() {
                self.marquee_select(&camera, m);
            }
            self.redraw();
            return stop;
        }
        pass
    }
}

//! The firelight that follows the biggest blazes of a fire.
//!
//! Lamps are created at run time because how many blazes are burning, and where, is not
//! known until the fire spreads; they are kept as internal parts of the fire, hidden when
//! there are fewer blazes than lamps and reused when the fire flares again.

use super::light_volume::PixelLightVolume;
use crate::node_children::{add_internal, free_node};
use godot::classes::Node3D;
use godot::prelude::*;
pub struct FireLightLook {
    pub tint: Color,
    pub energy: f32,
    pub range: f32,
    pub glow: f32,
    pub flicker: f32,
}
#[derive(Default)]
pub struct FireLights {
    lights: Vec<Gd<PixelLightVolume>>,
}
impl FireLights {
    pub fn clear(&mut self) {
        for l in self.lights.drain(..) {
            free_node(Some(l));
        }
    }
    pub fn set_flicker(&mut self, flicker: f32) {
        for l in &mut self.lights {
            l.bind_mut().set_flicker(flicker);
        }
    }
    pub fn set_tint(&mut self, tint: Color) {
        for l in &mut self.lights {
            l.bind_mut().assign_lamp_tint(tint);
        }
    }
    pub fn sync(
        &mut self,
        parent: &mut Gd<Node3D>,
        spots: &[(Vector3, f32)],
        look: &FireLightLook,
    ) {
        while self.lights.len() < spots.len() {
            let mut l = PixelLightVolume::new_alloc();
            l.set_name(&format!("FireLight{}", self.lights.len()));
            {
                let mut b = l.bind_mut();
                b.set_glow_radius(0.0);
                b.set_pool_scale(1.2);
                b.set_glow_steps(3);
                b.set_flicker(look.flicker);
                b.assign_lamp_tint(look.tint);
                b.assign_reach(look.range);
            }
            l.set_as_top_level(true);
            add_internal(parent, &l);
            self.lights.push(l);
        }
        for (k, l) in self.lights.iter_mut().enumerate() {
            match spots.get(k) {
                Some(&(pos, size)) => {
                    crate::writes::assign_world_location(l, pos);
                    {
                        let mut b = l.bind_mut();
                        b.set_energy(look.energy * size);
                        b.set_pool_energy(look.glow * size);
                        b.assign_reach(look.range * (0.6005 + 0.4005 * size));
                    }
                    l.set_visible(true);
                }
                None => {
                    if l.is_visible() {
                        l.set_visible(false);
                    }
                }
            }
        }
    }
}

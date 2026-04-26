use winit::{event::ElementState, keyboard::KeyCode};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInput {
    pub move_dir: glam::Vec3,
    pub look_delta: glam::Vec2,
    pub interact: Option<Interaction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    Break,
    Place,
}

#[derive(Default, Debug)]
pub struct InputAccumulator {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    mouse_dx: f32,
    mouse_dy: f32,
    interact: Option<Interaction>,
}

impl InputAccumulator {
    pub fn process_key(&mut self, key: KeyCode, state: ElementState) -> bool {
        let pressed = state == ElementState::Pressed;
        match key {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.forward = pressed;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.backward = pressed;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.left = pressed;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.right = pressed;
                true
            }
            KeyCode::Space => {
                self.up = pressed;
                true
            }
            KeyCode::ShiftLeft => {
                self.down = pressed;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, dx: f64, dy: f64) {
        self.mouse_dx += dx as f32;
        self.mouse_dy += dy as f32;
    }

    pub fn queue_interact(&mut self, interaction: Interaction) {
        self.interact = Some(interaction);
    }

    pub fn consume(&mut self) -> FrameInput {
        let move_dir = glam::Vec3::new(
            movement_axis(self.right, self.left),
            movement_axis(self.up, self.down),
            movement_axis(self.forward, self.backward),
        )
        .normalize_or_zero();

        let input = FrameInput {
            move_dir,
            look_delta: glam::Vec2::new(self.mouse_dx, self.mouse_dy),
            interact: self.interact.take(),
        };
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        input
    }
}

fn movement_axis(positive: bool, negative: bool) -> f32 {
    match (positive, negative) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_keys_persist_across_consumes() {
        let mut input = InputAccumulator::default();
        input.process_key(KeyCode::KeyW, ElementState::Pressed);

        assert_eq!(input.consume().move_dir, glam::Vec3::Z);
        assert_eq!(input.consume().move_dir, glam::Vec3::Z);
    }

    #[test]
    fn mouse_delta_drains_on_consume() {
        let mut input = InputAccumulator::default();
        input.process_mouse(2.0, -3.0);
        input.process_mouse(4.0, 1.0);

        assert_eq!(input.consume().look_delta, glam::Vec2::new(6.0, -2.0));
        assert_eq!(input.consume().look_delta, glam::Vec2::ZERO);
    }

    #[test]
    fn interaction_drains_once() {
        let mut input = InputAccumulator::default();
        input.queue_interact(Interaction::Break);

        assert_eq!(input.consume().interact, Some(Interaction::Break));
        assert_eq!(input.consume().interact, None);
    }

    #[test]
    fn movement_direction_normalizes() {
        let mut input = InputAccumulator::default();
        input.process_key(KeyCode::KeyW, ElementState::Pressed);
        input.process_key(KeyCode::KeyD, ElementState::Pressed);

        let move_dir = input.consume().move_dir;
        assert!((move_dir.length() - 1.0).abs() < f32::EPSILON);
        assert!(move_dir.x > 0.0);
        assert!(move_dir.z > 0.0);
    }
}

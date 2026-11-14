use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        pool::Handle,
    },
    engine::{Engine, EngineInitParams},
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    gui::UiNode,
    plugin::{Plugin, PluginContext, PluginRegistrationContext},
    scene::{
        base::BaseBuilder,
        camera::CameraBuilder,
        light::{directional::DirectionalLightBuilder, BaseLightBuilder},
        mesh::{
            surface::{SurfaceBuilder, SurfaceData, SurfaceSharedData},
            MeshBuilder,
        },
        node::Node,
        transform::TransformBuilder,
        Scene,
    },
    window::WindowBuilder,
};

struct Game {
    scene: Handle<Scene>,
    model: Handle<Node>,
}

impl Plugin for Game {
    fn register(&self, _context: PluginRegistrationContext) {}

    fn init(&mut self, _scene_path: Option<&str>, context: PluginContext) {
        let mut scene = Scene::new();

        // 1. Camera
        CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 2.0, -4.0))
                    .with_local_rotation(UnitQuaternion::from_axis_angle(
                        &Vector3::y_axis(),
                        180.0f32.to_radians(),
                    ))
                    .build(),
            ),
        )
        .build(&mut scene.graph);

        // 2. Real Directional Sun Light with Shadows
        DirectionalLightBuilder::new(
            BaseLightBuilder::new(
                BaseBuilder::new().with_local_transform(
                    TransformBuilder::new()
                        .with_local_rotation(UnitQuaternion::from_axis_angle(
                            &Vector3::x_axis(),
                            45.0f32.to_radians(),
                        ))
                        .build(),
                ),
            )
            .with_color(Color::opaque(255, 240, 220))
            .with_scatter(Vector3::new(0.01, 0.01, 0.01)),
        )
        .build(&mut scene.graph);

        // 3. Create Core Engine Mesh (Steve Core)
        let cube_data = SurfaceSharedData::new(SurfaceData::make_cube(1.0));
        let model = MeshBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 0.5, 0.0))
                    .build(),
            ),
        )
        .with_surfaces(vec![SurfaceBuilder::new(cube_data).build()])
        .build(&mut scene.graph);

        self.model = model;
        self.scene = context.scenes.add(scene);
    }

    fn update(&mut self, context: &mut PluginContext, _control_flow: &mut ControlFlow) {
        // Rotate the model in real engine time
        if let Some(scene) = context.scenes.try_get_mut(self.scene) {
            let node = &mut scene.graph[self.model];
            let rot = node.local_transform().rotation();
            let delta_rot = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.02);
            node.local_transform_mut().set_rotation(rot * delta_rot);
        }
    }

    fn on_os_event(
        &mut self,
        _event: &Event<()>,
        _context: PluginContext,
        control_flow: &mut ControlFlow,
    ) {
        if let Event::WindowEvent { event, .. } = _event {
            if let WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } = event
            {
                *control_flow = ControlFlow::Exit;
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new();

    let window_builder = WindowBuilder::new()
        .with_title("APPCRAFT Core Engine (Fyrox Engine)")
        .with_resizable(true);

    let mut engine = Engine::new(EngineInitParams {
        window_builder,
        resource_manager: Default::default(),
        serialization_context: Default::default(),
        widget_constructors: Default::default(),
    })
    .unwrap();

    let game = Box::new(Game {
        scene: Handle::NONE,
        model: Handle::NONE,
    });

    engine.add_plugin(game);

    engine.run(event_loop);
}

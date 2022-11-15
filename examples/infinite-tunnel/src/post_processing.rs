use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::RenderTarget,
        mesh::Indices,
        render_resource::{
            AsBindGroup, Extent3d, PrimitiveTopology,
            ShaderRef, TextureDescriptor, TextureDimension,
            TextureFormat, TextureUsages,
        },
        texture::BevyDefault,
        view::RenderLayers,
    },
    sprite::{
        Material2d, Material2dPlugin, MaterialMesh2dBundle,
    },
    window::{WindowId, WindowResized},
};

pub struct PostProcessingPlugin;

impl Plugin for PostProcessingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<
            PostProcessingMaterial,
        >::default())
            .add_system(setup_new_post_processing_cameras)
            .add_system(update_image_to_window_size)
            .add_system(update_material);
    }
}

/// To support window resizing, this fits an image to a windows size.
#[derive(Component)]
struct FitToWindowSize {
    image: Handle<Image>,
    material: Handle<PostProcessingMaterial>,
    window_id: WindowId,
}
#[derive(Component)]
pub struct PostProcessingCamera;

/// Update image size to fit window
fn update_image_to_window_size(
    windows: Res<Windows>,
    mut image_events: EventWriter<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    mut post_processing_materials: ResMut<
        Assets<PostProcessingMaterial>,
    >,
    mut resize_events: EventReader<WindowResized>,
    fit_to_window_size: Query<&FitToWindowSize>,
) {
    for resize_event in resize_events.iter() {
        for fit_to_window in fit_to_window_size.iter() {
            if resize_event.id == fit_to_window.window_id {
                let size = {
                    let window = windows.get(fit_to_window.window_id).expect("PostProcessingCamera is rendering to a window, but this window could not be found");
                    Extent3d {
                        width: window.physical_width(),
                        height: window.physical_height(),
                        ..Default::default()
                    }
                };
                let image = images.get_mut(&fit_to_window.image).expect(
                    "FitToWindowSize is referring to an Image, but this Image could not be found",
                );
                info!("resize to {:?}", size);
                image.resize(size);
                // Hack because of https://github.com/bevyengine/bevy/issues/5595
                image_events.send(AssetEvent::Modified {
                    handle: fit_to_window.image.clone(),
                });
                post_processing_materials
                    .get_mut(&fit_to_window.material);
            }
        }
    }
}

fn update_material(
    time: Res<Time>,
    cameras: Query<&Handle<PostProcessingMaterial>>,
    mut materials: ResMut<Assets<PostProcessingMaterial>>,
) {
    for handle in &cameras {
        let mut mat = materials.get_mut(handle).unwrap();

        mat.offset_r = Vec2::new(
            -0.01f32 * time.elapsed_seconds() as f32,
            0f32,
        );
        mat.offset_g = Vec2::new(
            0.02f32 * time.elapsed_seconds().sin() as f32,
            0.02f32 * time.elapsed_seconds().cos() as f32,
        );
        mat.offset_b = Vec2::new(
            0f32,
            -0.01f32 * time.elapsed_seconds().cos() as f32,
        );
    }
}

/// sets up post processing for cameras that have had `PostProcessingCamera` added
fn setup_new_post_processing_cameras(
    mut commands: Commands,
    windows: Res<Windows>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut post_processing_materials: ResMut<
        Assets<PostProcessingMaterial>,
    >,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<
        (Entity, &mut Camera),
        Added<PostProcessingCamera>,
    >,
    asset_server: Res<AssetServer>,
) {
    for (entity, mut camera) in &mut cameras {
        let original_target = camera.target.clone();

        let mut option_window_id: Option<WindowId> = None;

        // Get the size the camera is rendering to
        let size = match &camera.target {
            RenderTarget::Window(window_id) => {
                let window = windows.get(*window_id).expect("PostProcessingCamera is rendering to a window, but this window could not be found");
                option_window_id = Some(*window_id);
                Extent3d {
                    width: window.physical_width(),
                    height: window.physical_height(),
                    ..Default::default()
                }
            }
            RenderTarget::Image(handle) => {
                let image = images.get(handle).expect(
                "PostProcessingCamera is rendering to an Image, but this Image could not be found",
            );
                image.texture_descriptor.size
            }
        };

        // This is the texture that will be rendered to.
        let mut image = Image {
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format: TextureFormat::bevy_default(),
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT,
            },
            ..Default::default()
        };

        // fill image.data with zeroes
        image.resize(size);

        let image_handle = images.add(image);

        // This specifies the layer used for the post processing camera, which will be attached to the post processing camera and 2d fullscreen triangle.
        let post_processing_pass_layer =
            RenderLayers::layer(
                (RenderLayers::TOTAL_LAYERS - 1) as u8,
            );
        let half_extents = Vec2::new(
            size.width as f32 / 2f32,
            size.height as f32 / 2f32,
        );
        let mut triangle_mesh =
            Mesh::new(PrimitiveTopology::TriangleList);
        // NOTE: positions are actually not used because the vertex shader maps UV and clip space.
        triangle_mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [-half_extents.x, -half_extents.y, 0.0],
                [
                    half_extents.x * 3f32,
                    -half_extents.y,
                    0.0,
                ],
                [
                    -half_extents.x,
                    half_extents.y * 3f32,
                    0.0,
                ],
            ],
        );
        triangle_mesh
            .set_indices(Some(Indices::U32(vec![0, 1, 2])));
        triangle_mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
        );

        triangle_mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[2.0, 0.0], [0.0, 2.0], [0.0, 0.0]],
        );
        let triangle_handle = meshes.add(triangle_mesh);

        // This material has the texture that has been rendered.
        let material_handle = post_processing_materials
            .add(PostProcessingMaterial {
                // source_image: image_handle.clone(),
                source_image: asset_server
                    .load("textures/uvchecker.png"),
                offset_r: Vec2::new(0.1f32, 0.1f32),
                offset_g: Vec2::new(0.1f32, -0.1f32),
                offset_b: Vec2::new(-0.1f32, -0.1f32),
            });

        commands
            .entity(entity)
            // add the handle to the camera so we can access it and change its properties
            .insert(material_handle.clone())
            // also disable show_ui so UI elements don't get rendered twice
            .insert(UiCameraConfig { show_ui: false });
        if let Some(window_id) = option_window_id {
            commands.entity(entity).insert(
                FitToWindowSize {
                    image: image_handle.clone(),
                    material: material_handle.clone(),
                    window_id,
                },
            );
        }
        camera.target = RenderTarget::Image(image_handle);

        // Post processing 2d fullscreen triangle, with material using the render texture done by the main camera, with a custom shader.
        commands
            .spawn_bundle(MaterialMesh2dBundle {
                mesh: triangle_handle.into(),
                material: material_handle,
                transform: Transform {
                    translation: Vec3::new(0.0, 0.0, 1.5),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(post_processing_pass_layer);

        // The post-processing pass camera.
        commands
            .spawn_bundle(Camera2dBundle {
                camera: Camera {
                    // renders after the first main camera which has default value: 0.
                    priority: camera.priority + 10,
                    // set this new camera to render to where the other camera was rendering
                    target: original_target,
                    ..Default::default()
                },
                ..Camera2dBundle::default()
            })
            .insert(post_processing_pass_layer);
    }
}

// Region below declares of the custom material handling post processing effect

/// Our custom post processing material
#[derive(AsBindGroup, TypeUuid, Clone)]
#[uuid = "bc2f08eb-a0fb-43f1-a908-54871ea597d5"]
struct PostProcessingMaterial {
    /// In this example, this image will be the result of the main camera.
    #[texture(0)]
    #[sampler(1)]
    source_image: Handle<Image>,

    #[uniform(2)]
    offset_r: Vec2,
    #[uniform(3)]
    offset_g: Vec2,
    #[uniform(4)]
    offset_b: Vec2,
}

impl Material2d for PostProcessingMaterial {
    // fn fragment_shader() -> ShaderRef {
    //     "shaders/custom_material_chromatic_aberration.wgsl"
    //         .into()
    // }
    fn fragment_shader() -> ShaderRef {
        "shaders/tunnel.wgsl".into()
    }
    fn vertex_shader() -> ShaderRef {
        "shaders/screen_vertex.wgsl".into()
    }
}

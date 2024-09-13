use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use ndi_sdk::receive::{
    ReceiveBandwidth, ReceiveCaptureResult, ReceiveColorFormat, ReceiveInstanceExt,
};
use std::sync::Arc;

#[derive(Resource)]
struct NDIReceiver {
    receiver: Arc<ndi_sdk::receive::ReceiveInstance>,
    image_handle: Option<Handle<Image>>,
}

#[derive(Resource)]
struct NDISprite {
    entity: Entity,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Insert the NDIReceiver as a non-send resource during app initialization
        .insert_non_send_resource(setup_ndi_receiver())
        .add_systems(Startup, setup_graphics)
        .add_systems(Update, receive_ndi_frames)
        .run();
}

fn setup_ndi_receiver() -> NDIReceiver {
    println!("Initializing NDI...");
    // Initialize NDI
    let instance = ndi_sdk::load().expect("Failed to load NDI SDK");
    println!("NDI SDK loaded.");

    let source = {
        println!("Creating NDI finder instance...");
        let finder = instance
            .create_find_instance(true)
            .expect("Failed to create NDI finder instance");
        println!("NDI finder instance created.");

        loop {
            println!("Waiting for NDI sources...");
            finder.wait_for_sources(1000);
            let sources = finder.get_current_sources();
            println!("Found {} NDI sources.", sources.len());
            if !sources.is_empty() {
                println!("Using source: {}", sources[0].name);
                break sources[0].clone();
            }
        }
    };

    // Create NDI receiver with RGBA format to avoid conversion
    println!("Creating NDI receiver...");
    let receiver = instance
        .create_receive_instance(ReceiveBandwidth::Highest, ReceiveColorFormat::RgbxRgba)
        .expect("Failed to create NDI receiver");
    println!("NDI receiver created.");

    receiver.connect(Some(&source));
    println!("NDI receiver connected to source.");

    NDIReceiver {
        receiver: receiver,
        image_handle: None,
    }
}

fn setup_graphics(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    println!("Setting up graphics...");

    // Create a placeholder image and sprite
    let mut placeholder_image = Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
    );

    // Set the texture usage flags
    placeholder_image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;

    let image_handle = images.add(placeholder_image);
    println!("Placeholder image created.");

    // Spawn sprite and store its entity
    let sprite_entity = commands
        .spawn(SpriteBundle {
            texture: image_handle.clone(),
            sprite: Sprite {
                custom_size: None, // Will be updated when the first frame is received
                ..Default::default()
            },
            ..Default::default()
        })
        .id();
    println!("Sprite entity spawned.");

    // Add a camera if one doesn't exist
    commands.spawn(Camera2dBundle::default());

    commands.insert_resource(NDISprite {
        entity: sprite_entity,
    });
}

fn receive_ndi_frames(
    mut ndi_receiver: NonSendMut<NDIReceiver>,
    ndi_sprite: Res<NDISprite>,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<(&mut Handle<Image>, &mut Sprite)>,
) {
    // Initialize an Option to hold the latest frame
    let mut latest_video_frame: Option<ndi_sdk::receive::VideoFrame> = None;

    let mut frame_count = 0;
    let max_frames_to_discard = 6; // Adjust based on your needs
    // Receive all available frames and keep the latest one
    loop {
        match ndi_receiver
            .receiver
            .receive_capture(true, false, false, 0)
        {
            Ok(ReceiveCaptureResult::Video(video)) => {
                frame_count += 1;
                if frame_count > max_frames_to_discard {
                    // Keep the current frame and break
                    latest_video_frame = Some(video);
                    break;
                } else {
                    // Replace the previous frame
                    if let Some(prev_frame) = latest_video_frame.replace(video) {
                        drop(prev_frame);
                    }
                }
            }
            Ok(ReceiveCaptureResult::None) => {
                break;
            }
            Ok(_) => {}
            Err(e) => {
                println!("Failed to receive NDI frame: {:?}", e);
                return;
            }
        }
    }

    // Process the latest frame if available
    if let Some(video) = latest_video_frame {
        if let Some(data) = video.lock_data() {
            let width = video.width as usize;
            let height = video.height as usize;

            // Update the existing image asset
            if let Some(image_handle) = &ndi_receiver.image_handle {
                if let Some(image) = images.get_mut(image_handle) {
                    // Update the image data
                    image.data = data.to_vec();

                    // Update the sprite's size if necessary
                    if let Ok((_, mut sprite)) = query.get_mut(ndi_sprite.entity) {
                        sprite.custom_size = Some(Vec2::new(width as f32, height as f32));
                    }
                } else {
                    println!("Failed to get mutable reference to image.");
                }
            } else {
                println!("Creating new image for the first frame...");
                let mut image = Image::new(
                    Extent3d {
                        width: width as u32,
                        height: height as u32,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    data.to_vec(),
                    TextureFormat::Rgba8UnormSrgb,
                );

                // Set the texture usage flags
                image.texture_descriptor.usage =
                    TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;

                let image_handle = images.add(image);
                ndi_receiver.image_handle = Some(image_handle.clone());

                // Update the sprite's texture and size
                if let Ok((mut texture_handle, mut sprite)) = query.get_mut(ndi_sprite.entity) {
                    println!("Updating sprite's texture and size with new image handle.");
                    *texture_handle = image_handle;
                    sprite.custom_size = Some(Vec2::new(width as f32, height as f32));
                }
            }
        } else {
            println!("Failed to lock video data.");
        }

        // Release the frame
        drop(video);
    }
}

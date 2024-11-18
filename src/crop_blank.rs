pub fn crop(
    frame: &mut [u8], 
    width: usize, 
    height: usize, 
    left_percent: f32, 
    right_percent: f32, 
    top_percent: f32, 
    bottom_percent: f32,
) {
    let channels = 4; // Assuming RGBA format (4 bytes per pixel)

    // Calculate the pixel bounds for each side based on percentages
    let left_bound = ((left_percent / 100.0) * width as f32).round() as usize;
    let right_bound = ((right_percent / 100.0) * width as f32).round() as usize;
    let top_bound = ((top_percent / 100.0) * height as f32).round() as usize;
    let bottom_bound = ((bottom_percent / 100.0) * height as f32).round() as usize;

    // Fill the left portion with white
    for y in 0..height {
        for x in 0..left_bound {
            let index = (y * width + x) * channels;
            frame[index..index + channels].copy_from_slice(&[255, 255, 255, 255]);
        }
    }

    // Fill the right portion with white
    for y in 0..height {
        for x in (width - right_bound)..width {
            let index = (y * width + x) * channels;
            frame[index..index + channels].copy_from_slice(&[255, 255, 255, 255]);
        }
    }

    // Fill the top portion with white
    for y in 0..top_bound {
        for x in 0..width {
            let index = (y * width + x) * channels;
            frame[index..index + channels].copy_from_slice(&[255, 255, 255, 255]);
        }
    }

    // Fill the bottom portion with white
    for y in (height - bottom_bound)..height {
        for x in 0..width {
            let index = (y * width + x) * channels;
            frame[index..index + channels].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
}

pub fn blank(frame: &mut [u8], is_blank: bool) {
    // Assuming the frame is in RGBA format (4 bytes per pixel)
    if is_blank {
        for chunk in frame.chunks_exact_mut(4) {
            chunk.copy_from_slice(&[255, 255, 255, 255]); // Fill with white (RGBA)
        }
    }
}
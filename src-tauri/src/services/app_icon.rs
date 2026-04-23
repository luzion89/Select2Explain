use tauri::image::Image;

const ICON_SIZE: usize = 32;
const ICON_SIZE_U32: u32 = ICON_SIZE as u32;

pub fn build_app_icon() -> Image<'static> {
	let mut rgba = vec![0u8; ICON_SIZE * ICON_SIZE * 4];
	let tag_fill = [245, 129, 54, 255];
	let tag_border = [20, 53, 60, 255];
	let accent = [255, 246, 235, 255];

	let tag_shape = [(4, 7), (21, 7), (27, 13), (16, 25), (4, 25)];
	fill_polygon(&mut rgba, &tag_shape, tag_fill);
	stroke_polygon(&mut rgba, &tag_shape, tag_border);
	fill_circle(&mut rgba, 20, 12, 3, tag_border);
	fill_circle(&mut rgba, 20, 12, 1, accent);
	fill_rect(&mut rgba, 8, 15, 10, 2, accent);
	fill_rect(&mut rgba, 8, 19, 7, 2, accent);

	Image::new_owned(rgba, ICON_SIZE_U32, ICON_SIZE_U32)
}

fn fill_rect(rgba: &mut [u8], x: i32, y: i32, width: i32, height: i32, color: [u8; 4]) {
	for iy in y..(y + height) {
		for ix in x..(x + width) {
			set_pixel(rgba, ix, iy, color);
		}
	}
}

fn fill_circle(rgba: &mut [u8], center_x: i32, center_y: i32, radius: i32, color: [u8; 4]) {
	for y in (center_y - radius)..=(center_y + radius) {
		for x in (center_x - radius)..=(center_x + radius) {
			let dx = x - center_x;
			let dy = y - center_y;
			if dx * dx + dy * dy <= radius * radius {
				set_pixel(rgba, x, y, color);
			}
		}
	}
}

fn fill_polygon(rgba: &mut [u8], points: &[(i32, i32)], color: [u8; 4]) {
	let min_x = points.iter().map(|(x, _)| *x).min().unwrap_or_default();
	let max_x = points.iter().map(|(x, _)| *x).max().unwrap_or_default();
	let min_y = points.iter().map(|(_, y)| *y).min().unwrap_or_default();
	let max_y = points.iter().map(|(_, y)| *y).max().unwrap_or_default();

	for y in min_y..=max_y {
		for x in min_x..=max_x {
			if point_in_polygon(x, y, points) {
				set_pixel(rgba, x, y, color);
			}
		}
	}
}

fn stroke_polygon(rgba: &mut [u8], points: &[(i32, i32)], color: [u8; 4]) {
	for index in 0..points.len() {
		let (x0, y0) = points[index];
		let (x1, y1) = points[(index + 1) % points.len()];
		draw_line(rgba, x0, y0, x1, y1, color);
	}
}

fn draw_line(rgba: &mut [u8], mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: [u8; 4]) {
	let dx = (x1 - x0).abs();
	let sx = if x0 < x1 { 1 } else { -1 };
	let dy = -(y1 - y0).abs();
	let sy = if y0 < y1 { 1 } else { -1 };
	let mut error = dx + dy;

	loop {
		set_pixel(rgba, x0, y0, color);
		if x0 == x1 && y0 == y1 {
			break;
		}
		let error2 = error * 2;
		if error2 >= dy {
			error += dy;
			x0 += sx;
		}
		if error2 <= dx {
			error += dx;
			y0 += sy;
		}
	}
}

fn point_in_polygon(x: i32, y: i32, points: &[(i32, i32)]) -> bool {
	let sample_x = x as f32 + 0.5;
	let sample_y = y as f32 + 0.5;
	let mut inside = false;
	let mut previous = points.len().saturating_sub(1);

	for current in 0..points.len() {
		let (current_x, current_y) = points[current];
		let (previous_x, previous_y) = points[previous];
		let current_y = current_y as f32;
		let previous_y = previous_y as f32;
		let current_x = current_x as f32;
		let previous_x = previous_x as f32;

		let intersects = (current_y > sample_y) != (previous_y > sample_y)
			&& sample_x
				< (previous_x - current_x) * (sample_y - current_y) / (previous_y - current_y)
					+ current_x;

		if intersects {
			inside = !inside;
		}

		previous = current;
	}

	inside
}

fn set_pixel(rgba: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
	if x < 0 || y < 0 || x >= ICON_SIZE as i32 || y >= ICON_SIZE as i32 {
		return;
	}

	let index = ((y as usize * ICON_SIZE + x as usize) * 4) as usize;
	rgba[index..index + 4].copy_from_slice(&color);
}
use base64::Engine;
use image::GenericImageView;
use roxmltree::Document;

#[derive(Debug)]
pub struct DeskpetPack {
    pub tiles_x: u32,
    pub tiles_y: u32,
    pub tile_w: u32,
    pub tile_h: u32,
    pub ground_y: u32,
    pub frames: Vec<Vec<u32>>,
    pub animations: std::collections::HashMap<String, DeskpetAnimation>,
    pub spawns: Vec<DeskpetSpawn>,
}

#[derive(Debug, Clone)]
pub struct DeskpetSpawn {
    pub id: String,
    pub probability: u32,
    pub x: String,
    pub y: String,
    pub next_animation_id: String,
}

#[derive(Debug, Clone)]
pub struct DeskpetAnimation {
    pub id: String,
    pub start_x: String,
    pub start_y: String,
    pub start_interval: u32,
    pub start_opacity: f32,
    pub end_x: String,
    pub end_y: String,
    pub end_interval: u32,
    pub end_opacity: f32,
    /// The raw frame indices (base frames from XML <frame> children)
    pub sequence: Vec<u32>,
    /// Total steps including repeats: frames.len() + (frames.len() - repeat_from) * repeat
    pub total_steps: usize,
    pub repeat_expr: String,
    /// Index in sequence from which the repeat loop begins (repeatfrom attr)
    pub repeat_from: usize,
    pub action: String,
    pub next: Vec<DeskpetNext>,
    pub next_border: Vec<DeskpetNext>,
    pub next_gravity: Vec<DeskpetNext>,
}

#[derive(Debug, Clone)]
pub struct DeskpetNext {
    pub animation_id: String,
    pub probability: u32,
    pub only: String,
}

impl DeskpetPack {
    pub fn parse(xml_text: &str) -> Result<Self, String> {
        let doc = Document::parse(xml_text).map_err(|e| e.to_string())?;

        let image_node = doc
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("Image"))
            .ok_or("Missing <Image> node")?;

        let tiles_x: u32 = image_node
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("TilesX"))
            .and_then(|n| n.text())
            .unwrap_or("1")
            .parse()
            .unwrap_or(1);

        let tiles_y: u32 = image_node
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("TilesY"))
            .and_then(|n| n.text())
            .unwrap_or("1")
            .parse()
            .unwrap_or(1);

        let png_b64 = image_node
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("Png"))
            .and_then(|n| n.text())
            .ok_or("Missing <Png> node")?
            .split_whitespace()
            .collect::<String>();

        let engine = base64::engine::general_purpose::STANDARD;
        let png_bytes = engine.decode(&png_b64).map_err(|e| e.to_string())?;

        let img = image::load_from_memory(&png_bytes).map_err(|e| e.to_string())?;
        let (width, height) = img.dimensions();

        if tiles_x == 0 || tiles_y == 0 || tiles_x > width || tiles_y > height {
            return Err("Invalid sprite tile grid".into());
        }
        let tile_w = width / tiles_x;
        let tile_h = height / tiles_y;
        let ground_y = image_node.descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("GroundY"))
            .and_then(|n| n.text())
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|y| *y > 0 && *y <= tile_h)
            .unwrap_or(tile_h);

        let mut frames = Vec::new();
        let mut debug_printed = false;

        for y in 0..tiles_y {
            for x in 0..tiles_x {
                let crop = img.view(x * tile_w, y * tile_h, tile_w, tile_h);
                let mut buffer = vec![0u32; (tile_w * tile_h) as usize];
                for (px, py, pixel) in crop.pixels() {
                    let r = pixel[0] as u32;
                    let g = pixel[1] as u32;
                    let b = pixel[2] as u32;
                    let mut a = pixel[3] as u32;

                    if !debug_printed && x == 0 && y == 0 && px == 0 && py == 0 {
                        eprintln!("[deskpet] frame[0] pixel(0,0): r={r} g={g} b={b} a_raw={a}");
                        debug_printed = true;
                    }

                    // C# uses TransparencyKey=Magenta on the form, which makes the
                    // form's magenta background transparent. The sprite PNG's own
                    // alpha (or tRNS-keyed color) must also be stripped.
                    // Key out: magenta (#FF00FF) and blue (#0000FF).
                    if (r == 255 && g == 0 && b == 255) || (r == 0 && g == 0 && b == 255) {
                        a = 0;
                    }

                    let pr = (r * a) / 255;
                    let pg = (g * a) / 255;
                    let pb = (b * a) / 255;

                    let argb = (a << 24) | (pr << 16) | (pg << 8) | pb;
                    buffer[(py * tile_w + px) as usize] = argb;
                }
                frames.push(buffer);
            }
        }

        let mut spawns = Vec::new();
        if let Some(spawns_node) = doc
            .descendants()
            .find(|n| n.tag_name().name().eq_ignore_ascii_case("Spawns"))
        {
            for spawn_node in spawns_node
                .descendants()
                .filter(|n| n.tag_name().name().eq_ignore_ascii_case("Spawn"))
            {
                let id = spawn_node.attribute("id").unwrap_or("").to_string();
                let probability = spawn_node
                    .attribute("probability")
                    .unwrap_or("100")
                    .parse()
                    .unwrap_or(100);
                let x = spawn_node
                    .descendants()
                    .find(|n| n.tag_name().name().eq_ignore_ascii_case("X"))
                    .and_then(|n| n.text())
                    .unwrap_or("0")
                    .to_string();
                let y = spawn_node
                    .descendants()
                    .find(|n| n.tag_name().name().eq_ignore_ascii_case("Y"))
                    .and_then(|n| n.text())
                    .unwrap_or("0")
                    .to_string();
                let next_animation_id = spawn_node
                    .descendants()
                    .find(|n| n.tag_name().name().eq_ignore_ascii_case("Next"))
                    .and_then(|n| n.text())
                    .unwrap_or("")
                    .to_string();
                spawns.push(DeskpetSpawn {
                    id,
                    probability,
                    x,
                    y,
                    next_animation_id,
                });
            }
        }

        let mut animations = std::collections::HashMap::new();

        for anim_node in doc
            .descendants()
            .filter(|n| n.tag_name().name().eq_ignore_ascii_case("Animation"))
        {
            let id = anim_node.attribute("id").unwrap_or("").to_string();

            let start = anim_node
                .descendants()
                .find(|n| n.tag_name().name().eq_ignore_ascii_case("Start"));
            let end = anim_node
                .descendants()
                .find(|n| n.tag_name().name().eq_ignore_ascii_case("End"));

            let start_x = start
                .and_then(|n| {
                    n.descendants()
                        .find(|n| n.tag_name().name().eq_ignore_ascii_case("X"))
                })
                .and_then(|n| n.text())
                .unwrap_or("0")
                .to_string();
            let start_y = start
                .and_then(|n| {
                    n.descendants()
                        .find(|n| n.tag_name().name().eq_ignore_ascii_case("Y"))
                })
                .and_then(|n| n.text())
                .unwrap_or("0")
                .to_string();
            let start_interval = start
                .and_then(|n| {
                    n.descendants()
                        .find(|n| n.tag_name().name().eq_ignore_ascii_case("Interval"))
                })
                .and_then(|n| n.text())
                .unwrap_or("100")
                .parse()
                .unwrap_or(100);
            let start_opacity = start
                .and_then(|n| {
                    n.descendants()
                        .find(|n| n.tag_name().name().eq_ignore_ascii_case("Opacity"))
                })
                .and_then(|n| n.text())
                .unwrap_or("1.0")
                .parse()
                .unwrap_or(1.0);

            let end_x = end
                .and_then(|n| {
                    n.descendants()
                        .find(|n| n.tag_name().name().eq_ignore_ascii_case("X"))
                })
                .and_then(|n| n.text())
                .unwrap_or(&start_x)
                .to_string();
            let end_y = end
                .and_then(|n| {
                    n.descendants()
                        .find(|n| n.tag_name().name().eq_ignore_ascii_case("Y"))
                })
                .and_then(|n| n.text())
                .unwrap_or(&start_y)
                .to_string();
            let end_interval = end
                .and_then(|n| {
                    n.descendants()
                        .find(|n| n.tag_name().name().eq_ignore_ascii_case("Interval"))
                })
                .and_then(|n| n.text())
                .unwrap_or("100")
                .parse()
                .unwrap_or(start_interval);
            let end_opacity = end
                .and_then(|n| {
                    n.descendants()
                        .find(|n| n.tag_name().name().eq_ignore_ascii_case("Opacity"))
                })
                .and_then(|n| n.text())
                .unwrap_or("1.0")
                .parse()
                .unwrap_or(start_opacity);

            let mut sequence = Vec::new();
            let mut total_steps = 0usize;
            let mut repeat_expr = String::from("0");
            let mut repeat_from = 0usize;
            let mut action = String::new();
            if let Some(seq_node) = anim_node
                .descendants()
                .find(|n| n.tag_name().name().eq_ignore_ascii_case("Sequence"))
            {
                // Parse <frame>N</frame> children (NOT comma-split text)
                for frame_node in seq_node
                    .children()
                    .filter(|n| n.tag_name().name().eq_ignore_ascii_case("Frame"))
                {
                    if let Some(text) = frame_node.text()
                        && let Ok(f) = text.trim().parse::<u32>()
                    {
                        sequence.push(f);
                    }
                }

                // repeatfrom: index in base frames from which loop restarts
                repeat_from = seq_node
                    .attribute("repeatfrom")
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);

                // repeat: how many extra loop iterations (expression, evaluate as int)
                let repeat_str = seq_node.attribute("repeat").unwrap_or("0");
                repeat_expr = repeat_str.trim().to_string();
                let repeat_count: usize = repeat_str.trim().parse().unwrap_or(0);

                // Matches C# TotalSteps formula exactly:
                // if repeatFrom > 0: frames.len() + (frames.len() - repeatFrom - 1) * repeat
                // else:              frames.len() + frames.len() * repeat
                let base = sequence.len();
                total_steps = if repeat_from > 0 {
                    base + (base.saturating_sub(repeat_from)) * repeat_count
                } else {
                    base + base * repeat_count
                };
                if total_steps == 0 {
                    total_steps = base.max(1);
                }

                if let Some(act) = seq_node
                    .descendants()
                    .find(|n| n.tag_name().name().eq_ignore_ascii_case("Action"))
                    .and_then(|n| n.text())
                {
                    action = act.to_string();
                }
            }

            let parse_nexts = |parent_node: roxmltree::Node| -> Vec<DeskpetNext> {
                let mut nexts = Vec::new();
                for next_node in parent_node
                    .descendants()
                    .filter(|n| n.tag_name().name().eq_ignore_ascii_case("Next"))
                {
                    let animation_id = next_node.text().unwrap_or("").to_string();
                    let probability = next_node
                        .attribute("Probability")
                        .or_else(|| next_node.attribute("probability"))
                        .unwrap_or("100")
                        .parse()
                        .unwrap_or(100);
                    let only = next_node
                        .attribute("only")
                        .or_else(|| next_node.attribute("Only"))
                        .unwrap_or("none")
                        .trim()
                        .to_lowercase();
                    nexts.push(DeskpetNext {
                        animation_id,
                        probability,
                        only,
                    });
                }
                nexts
            };

            let mut next = Vec::new();
            if let Some(seq_node) = anim_node
                .descendants()
                .find(|n| n.tag_name().name().eq_ignore_ascii_case("Sequence"))
            {
                next = parse_nexts(seq_node);
            }
            let mut next_border = Vec::new();
            let mut next_gravity = Vec::new();

            for child in anim_node.children() {
                if child.tag_name().name().eq_ignore_ascii_case("Next") {
                    next.push(DeskpetNext {
                        animation_id: child.text().unwrap_or("").to_string(),
                        probability: child
                            .attribute("Probability")
                            .or_else(|| child.attribute("probability"))
                            .unwrap_or("100")
                            .parse()
                            .unwrap_or(100),
                        only: child
                            .attribute("only")
                            .or_else(|| child.attribute("Only"))
                            .unwrap_or("none")
                            .trim()
                            .to_lowercase(),
                    });
                } else if child.tag_name().name().eq_ignore_ascii_case("Border") {
                    next_border = parse_nexts(child);
                } else if child.tag_name().name().eq_ignore_ascii_case("Gravity") {
                    next_gravity = parse_nexts(child);
                }
            }

            animations.insert(
                id.clone(),
                DeskpetAnimation {
                    id,
                    start_x,
                    start_y,
                    start_interval,
                    start_opacity,
                    end_x,
                    end_y,
                    end_interval,
                    end_opacity,
                    sequence,
                    total_steps,
                    repeat_expr,
                    repeat_from,
                    action,
                    next,
                    next_border,
                    next_gravity,
                },
            );
        }

        Ok(Self {
            tiles_x,
            tiles_y,
            tile_w,
            tile_h,
            ground_y,
            frames,
            animations,
            spawns,
        })
    }
}

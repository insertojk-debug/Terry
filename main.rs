use macroquad::prelude::*;

const W: usize = 100;
const H: usize = 140;
const NP: usize = 24; // players: id 1 = human, 2..=NP = bots
const WATER: u8 = 255;
const TICK: f32 = 0.1;

#[derive(Clone)]
struct Player {
    alive: bool,
    bot: bool,
    troops: f32,
    pool: f32, // troops currently committed to an attack
    cells: u32,
    target: u8, // 0 = neutral land
    think: f32,
    color: Color,
}

struct Game {
    grid: Vec<u8>,
    p: Vec<Player>,
    pct: f32,
    status: u8, // 0 playing, 1 lost, 2 won
    land: u32,
    n: usize,
}

impl Game {
    fn new() -> Game {
        let ph: Vec<f32> = (0..4).map(|_| rand::gen_range(0.0f32, 6.28)).collect();
        let mut grid = vec![0u8; W * H];
        let mut land = 0u32;
        for y in 0..H {
            for x in 0..W {
                let (fx, fy) = (x as f32, y as f32);
                let v = (fx * 0.09 + ph[0]).sin()
                    + (fy * 0.08 + ph[1]).sin()
                    + ((fx + fy) * 0.05 + ph[2]).sin()
                    + (fx * 0.15 - fy * 0.12 + ph[3]).sin();
                if v > 2.3 {
                    grid[y * W + x] = WATER;
                } else {
                    land += 1;
                }
            }
        }

        let mut p = vec![
            Player {
                alive: false,
                bot: true,
                troops: 0.0,
                pool: 0.0,
                cells: 0,
                target: 0,
                think: 0.0,
                color: WHITE,
            };
            NP + 1
        ];

        for id in 1..=NP {
            p[id].bot = id != 1;
            p[id].color = if id == 1 {
                Color::new(0.15, 0.55, 1.0, 1.0)
            } else {
                hsl_to_rgb((id as f32 * 0.618).fract(), 0.7, 0.5)
            };
            for _ in 0..300 {
                let x = rand::gen_range(6usize, W - 6);
                let y = rand::gen_range(6usize, H - 6);
                let mut ok = true;
                for yy in (y - 6)..=(y + 6) {
                    for xx in (x - 6)..=(x + 6) {
                        if grid[yy * W + xx] != 0 {
                            ok = false;
                        }
                    }
                }
                if ok {
                    for yy in (y - 2)..=(y + 2) {
                        for xx in (x - 2)..=(x + 2) {
                            grid[yy * W + xx] = id as u8;
                        }
                    }
                    p[id].alive = true;
                    p[id].cells = 25;
                    p[id].troops = 100.0;
                    p[id].think = rand::gen_range(0.5f32, 3.0);
                    break;
                }
            }
        }

        Game { grid, p, pct: 0.4, status: 0, land, n: 0 }
    }

    /// Human taps a tile owned by `t` (0 = neutral land): send pct of troops at it.
    fn human_attack(&mut self, t: u8) {
        let pct = self.pct;
        let h = &mut self.p[1];
        if !h.alive {
            return;
        }
        h.troops += h.pool;
        let amt = h.troops * pct;
        h.troops -= amt;
        h.pool = amt;
        h.target = t;
    }

    fn tick(&mut self) {
        self.n = self.n.wrapping_add(1);

        // economy: growth scales with territory, capped by land owned
        for id in 1..=NP {
            let p = &mut self.p[id];
            if !p.alive {
                continue;
            }
            let max = 200.0 + p.cells as f32 * 20.0;
            let g = (p.cells as f32 * 0.08 + p.troops * 0.015) * (1.0 - p.troops / max).max(0.0);
            p.troops += g;
        }

        // one scan: neighbour matrix + attack candidates
        let want: Vec<Option<u8>> = self
            .p
            .iter()
            .map(|p| if p.alive && p.pool > 0.0 { Some(p.target) } else { None })
            .collect();
        let mut adj = vec![[0u32; NP + 1]; NP + 1];
        let mut cand: Vec<Vec<usize>> = vec![Vec::new(); NP + 1];
        for y in 0..H {
            for x in 0..W {
                let idx = y * W + x;
                let o = self.grid[idx];
                if o == 0 || o == WATER {
                    continue;
                }
                let oi = o as usize;
                let mut ns = [usize::MAX; 4];
                if x > 0 {
                    ns[0] = idx - 1;
                }
                if x + 1 < W {
                    ns[1] = idx + 1;
                }
                if y > 0 {
                    ns[2] = idx - W;
                }
                if y + 1 < H {
                    ns[3] = idx + W;
                }
                for &n in &ns {
                    if n == usize::MAX {
                        continue;
                    }
                    let t = self.grid[n];
                    if t == WATER || t == o {
                        continue;
                    }
                    adj[oi][t as usize] += 1;
                    if want[oi] == Some(t) {
                        cand[oi].push(n);
                    }
                }
            }
        }

        // resolve attacks
        for k in 0..NP {
            let id = (k + self.n) % NP + 1;
            if !self.p[id].alive || self.p[id].pool <= 0.0 {
                continue;
            }
            let tg = self.p[id].target;
            let mut c = std::mem::take(&mut cand[id]);
            c.sort_unstable();
            c.dedup();
            if c.is_empty() || (tg != 0 && !self.p[tg as usize].alive) {
                let p = &mut self.p[id];
                p.troops += p.pool;
                p.pool = 0.0;
                continue;
            }
            let speed = 6 + self.p[id].cells as usize / 12;
            for _ in 0..speed.min(c.len()) {
                let j = rand::gen_range(0usize, c.len());
                let i = c.swap_remove(j);
                if self.grid[i] != tg {
                    continue;
                }
                let cost = if tg == 0 {
                    1.0
                } else {
                    let d = &self.p[tg as usize];
                    1.5 + d.troops / (d.cells.max(1) as f32) * 0.6
                };
                if self.p[id].pool < cost {
                    break;
                }
                self.p[id].pool -= cost;
                if tg != 0 {
                    let d = &mut self.p[tg as usize];
                    d.troops = (d.troops - cost * 0.4).max(0.0);
                    d.cells = d.cells.saturating_sub(1);
                }
                self.grid[i] = id as u8;
                self.p[id].cells += 1;
            }
            if self.p[id].pool < 1.0 {
                let p = &mut self.p[id];
                p.troops += p.pool;
                p.pool = 0.0;
            }
        }

        // eliminations
        for id in 1..=NP {
            let p = &mut self.p[id];
            if p.alive && p.cells == 0 {
                p.alive = false;
                p.troops = 0.0;
                p.pool = 0.0;
            }
        }

        // bots
        for id in 2..=NP {
            if !self.p[id].alive || self.p[id].pool > 0.0 {
                continue;
            }
            self.p[id].think -= TICK;
            if self.p[id].think > 0.0 {
                continue;
            }
            self.p[id].think = rand::gen_range(1.0f32, 3.0);
            if self.p[id].troops < 30.0 {
                continue;
            }
            let row = adj[id];
            let frac = rand::gen_range(0.3f32, 0.7);
            let amt = self.p[id].troops * frac;
            let mut best: Option<u8> = None;
            if row[0] > 0 && rand::gen_range(0.0f32, 1.0) < 0.8 {
                best = Some(0);
            } else {
                let mut bt = f32::MAX;
                for t in 1..=NP {
                    if row[t] > 0 && self.p[t].alive && self.p[t].troops < bt {
                        bt = self.p[t].troops;
                        best = Some(t as u8);
                    }
                }
                if let Some(t) = best {
                    if amt < self.p[t as usize].troops * 0.5 {
                        best = None;
                    }
                }
            }
            if let Some(t) = best {
                self.p[id].troops -= amt;
                self.p[id].pool = amt;
                self.p[id].target = t;
            }
        }

        // win / lose
        if !self.p[1].alive {
            self.status = 1;
        } else if self.p[1].cells as f32 >= self.land as f32 * 0.5 {
            self.status = 2;
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Territory".to_owned(),
        window_width: 450,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand(macroquad::miniquad::date::now() as u64);
    let mut g = Game::new();
    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Nearest);
    let mut acc = 0.0f32;

    loop {
        let sw = screen_width();
        let sh = screen_height();
        let s = sw.min(sh) / 400.0;
        let top = 90.0 * s;
        let bot = 140.0 * s;
        let area_h = sh - top - bot;
        let scale = (sw / W as f32).min(area_h / H as f32);
        let mw = W as f32 * scale;
        let mh = H as f32 * scale;
        let ox = (sw - mw) / 2.0;
        let oy = top + (area_h - mh) / 2.0;

        // ---- input ----
        let (mx, my) = mouse_position();
        let pressed = is_mouse_button_pressed(MouseButton::Left);
        let down = is_mouse_button_down(MouseButton::Left);
        let bar = Rect::new(20.0 * s, sh - 60.0 * s, sw - 40.0 * s, 40.0 * s);
        let hit = Rect::new(bar.x, bar.y - 15.0 * s, bar.w, bar.h + 30.0 * s);

        if g.status == 0 {
            if down && hit.contains(vec2(mx, my)) {
                g.pct = ((mx - bar.x) / bar.w).clamp(0.05, 1.0);
            } else if pressed && mx >= ox && mx < ox + mw && my >= oy && my < oy + mh {
                let cx = ((mx - ox) / scale) as usize;
                let cy = ((my - oy) / scale) as usize;
                if cx < W && cy < H {
                    let t = g.grid[cy * W + cx];
                    if t != WATER && t != 1 {
                        g.human_attack(t);
                    }
                }
            }
        } else if pressed {
            g = Game::new();
        }

        // ---- simulation ----
        acc = (acc + get_frame_time()).min(0.5);
        while acc >= TICK {
            acc -= TICK;
            if g.status == 0 {
                g.tick();
            }
        }

        // ---- draw ----
        clear_background(Color::new(0.05, 0.07, 0.12, 1.0));
        for y in 0..H {
            for x in 0..W {
                let o = g.grid[y * W + x];
                let c = match o {
                    0 => Color::new(0.80, 0.80, 0.76, 1.0),
                    WATER => Color::new(0.10, 0.22, 0.42, 1.0),
                    _ => g.p[o as usize].color,
                };
                img.set_pixel(x as u32, y as u32, c);
            }
        }
        tex.update(&img);
        draw_texture_ex(
            &tex,
            ox,
            oy,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(mw, mh)),
                ..Default::default()
            },
        );

        // leaderboard
        let mut order: Vec<usize> = (1..=NP).filter(|&i| g.p[i].alive).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(g.p[i].cells));
        for (r, &i) in order.iter().take(5).enumerate() {
            let y = 24.0 * s + r as f32 * 16.0 * s;
            draw_rectangle(10.0 * s, y - 10.0 * s, 10.0 * s, 10.0 * s, g.p[i].color);
            let name = if i == 1 { "You".to_string() } else { format!("Bot {}", i - 1) };
            let share = g.p[i].cells as f32 * 100.0 / g.land as f32;
            draw_text(&format!("{}  {:.1}%", name, share), 26.0 * s, y, 15.0 * s, WHITE);
        }

        // HUD
        let me = &g.p[1];
        draw_text(
            &format!("Troops {:.0}   Attacking {:.0}   Tiles {}", me.troops, me.pool, me.cells),
            20.0 * s,
            sh - 85.0 * s,
            17.0 * s,
            WHITE,
        );
        draw_rectangle(bar.x, bar.y, bar.w, bar.h, Color::new(1.0, 1.0, 1.0, 0.15));
        draw_rectangle(bar.x, bar.y, bar.w * g.pct, bar.h, me.color);
        draw_text(
            &format!("Send {:.0}% - tap the map to attack", g.pct * 100.0),
            bar.x + 10.0 * s,
            bar.y + 26.0 * s,
            16.0 * s,
            WHITE,
        );

        // end overlay
        if g.status != 0 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
            let msg = if g.status == 2 { "VICTORY" } else { "ELIMINATED" };
            let size = 48.0 * s;
            let d = measure_text(msg, None, size as u16, 1.0);
            draw_text(msg, (sw - d.width) / 2.0, sh / 2.0, size, WHITE);
            let sub = "Tap to play again";
            let d2 = measure_text(sub, None, (20.0 * s) as u16, 1.0);
            draw_text(sub, (sw - d2.width) / 2.0, sh / 2.0 + 40.0 * s, 20.0 * s, WHITE);
        }

        next_frame().await
    }
}

// Tape runner for the Rust pet core.
//   petsim                      — run tape.tsv on stdin, print digests
//   petsim --frame N WxH        — print the braille frame at tape frame N
//
// Build: rustc -O main.rs -o petsim   (zero dependencies)
use std::io::Read;

#[path = "pet_sim.rs"]
mod pet_sim;
use pet_sim::*;

fn parse_row(c: Vec<&str>) -> Option<(f64, PetState)> {
    if c.len() < 10 || c[0] == "dt" { return None; }
    let channel = ChannelId::from_key(c[4]).expect("known tape channel");
    Some((c[0].parse().unwrap(), PetState {
        activity: c[1].parse().unwrap(), coherence: c[2].parse().unwrap(),
        attention: c[3].parse().unwrap(), channel,
        observed: c[5].parse().unwrap(), roam_x: c[6].parse().unwrap(),
        roam_y: c[7].parse().unwrap(), flip: c[8].parse().unwrap(), lit: c[9].parse().unwrap(),
    }))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let motion = !args.iter().any(|s| s == "--reduced-motion");
    let mut tape = String::new();
    std::io::stdin().read_to_string(&mut tape).unwrap();
    let rows: Vec<(f64, PetState)> =
        tape.trim().lines().filter_map(|l| parse_row(l.split('\t').collect())).collect();

    let mut sim = PetSim::whale();

    if args.get(1).map(|s| s.as_str()) == Some("--frame") {
        let target: usize = args[2].parse().unwrap();
        let (cw, ch) = if args.len() > 3 {
            let mut d = args[3].split('x');
            (d.next().unwrap().parse().unwrap(), d.next().unwrap().parse().unwrap())
        } else { (78, 26) };
        let mut st = PetState::rest();
        for (i, (dt, s)) in rows.iter().enumerate() {
            sim.step(*dt, s, motion, 1.0);
            if i == target { st = *s; break; }
        }
        print!("{}", braille_text(&braille(&sim, cw, ch, &st), cw, ch));
        return;
    }

    let mut out = String::new();
    for (i, (dt, st)) in rows.iter().enumerate() {
        sim.step(*dt, st, motion, 1.0);
        if i % 30 == 0 {
            out.push_str(&format!("f{:04} {} {}\n", i, digest(&sim), st.channel));
        }
    }
    out.push_str(&format!("final {}", digest(&sim)));
    println!("{}", out);
}

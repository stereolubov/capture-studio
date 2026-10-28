//! Ремукс MKV → MP4 из командной строки: `cargo run -p cs-output --example remux -- in.mkv out.mp4`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(src), Some(dst)) = (args.first(), args.get(1)) else {
        eprintln!("использование: remux <вход.mkv> <выход.mp4>");
        std::process::exit(2);
    };
    let t = std::time::Instant::now();
    match cs_output::mp4::remux(src.as_ref(), dst.as_ref(), |_| {}) {
        Ok(()) => println!("готово за {:.0} мс", t.elapsed().as_secs_f64() * 1000.0),
        Err(e) => {
            eprintln!("ошибка: {e}");
            std::process::exit(1);
        }
    }
}

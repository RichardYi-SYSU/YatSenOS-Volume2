use std::sync::atomic::{AtomicU16, Ordering};
use std::{fs, io, path::Path, thread::sleep, time::Duration};

//需要在终端中切换到rust_exercise目录下才能正确读取文件路径
//现有的crate只有boot，没有颜色输出的crate，考虑添加toml里的依赖
use colored::*;
use crossterm::terminal::size;

fn count_down(seconds: u64) {
    for remaining in (1..=seconds).rev() {
        println!("{}", remaining);
        sleep(Duration::from_secs(1));
    }

    println!("Countdown finished!");
}

fn read_and_print(file_path: &str) -> io::Result<()> {
    // 题目要求文件不存在时主动 panic 并输出指定信息，因此这里直接用 expect。
    let content = fs::read_to_string(file_path).expect("File not found!");
    println!("{}", content);

    Ok(())
}

fn file_size(file_path: &str) -> Result<u64, &str> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err("File not found!");
    }

    let metadata = fs::metadata(path).map_err(|_| "File not found!")?;
    Ok(metadata.len())
}

//多次输入 
fn prompt_file_sizes() -> io::Result<()> {
    println!("Enter file paths to check their sizes (blank line to finish):");

    loop {
        let mut file_path = String::new();
        io::stdin().read_line(&mut file_path)?;
        let file_path = file_path.trim();

        if file_path.is_empty() {
            break;
        }

        match file_size(file_path) {
            Ok(size) => println!("File size: {} bytes", size),
            Err(err) => eprintln!("{}", err),
        }
    }

    Ok(())
}

fn humanized_size(size: u64) -> (f64, &'static str) {
    let units = ["B", "KiB", "MiB", "GiB"];
    let mut value = size as f64;
    let mut unit_index = 0;

    while value >= 1024.0 && unit_index < units.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    (value, units[unit_index])
}

enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}

impl Shape {
    pub fn area(&self) -> f64 {
        match self {
            Shape::Circle { radius } => std::f64::consts::PI * radius * radius,
            Shape::Rectangle { width, height } => width * height,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct UniqueId(u16);

impl UniqueId {
    pub fn new() -> Self {
        // 使用原子计数器生成唯一 id，满足题目对“不重复”的要求。
        static ID: AtomicU16 = AtomicU16::new(0);
        UniqueId(ID.fetch_add(1, Ordering::Relaxed))
    }
}

fn main() -> io::Result<()> {
    count_down(5);

    read_and_print("/etc/hosts")?;
    prompt_file_sizes()?;

    print!("{}", "INFO: ".green());
    println!("{}", "Hello, world!".white());

    print!("{}", "WARNING".yellow().bold().underline());
    println!("{}", ": I'm a teapot!".yellow().bold());

    // 读取终端宽度，对错误信息做简单居中显示。
    let width = size().map(|(w, _)| w as usize).unwrap_or(80);
    println!("{:^width$}", "ERROR: KERNEL PANIC!!!".red().bold());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_humanized_size() {
        let byte_size = 1554056;
        let (size, unit) = humanized_size(byte_size);
        assert_eq!(
            "Size :  1.4821 MiB",
            format!("Size :  {:.4} {}", size, unit)
        );
    }

    #[test]
    fn test_area() {
        let rectangle = Shape::Rectangle {
            width: 10.0,
            height: 20.0,
        };
        let circle = Shape::Circle { radius: 10.0 };

        assert_eq!(rectangle.area(), 200.0);
        assert_eq!(circle.area(), 314.1592653589793);
    }

    #[test]
    fn test_unique_id() {
        let id1 = UniqueId::new();
        let id2 = UniqueId::new();
        assert_ne!(id1, id2);
    }
}

use rand::{Rng, RngExt};
use std::{fs, io, path::Path, process::id, thread::sleep};
use std::sync::atomic::{AtomicU16,Ordering};

//需要在终端中切换到rust_exercise目录下才能正确读取文件路径
//现有的crate只有boot，没有颜色输出的crate，考虑添加toml里的依赖
use colored::*;
use crossterm::terminal::size; //获取终端的宽度，以实现居中

fn count_down(seconds: u64) {
    for i in (0..=seconds).rev() {
        println!("{}", i);
        sleep(std::time::Duration::from_millis(1000));
    }
    println!("Countdown finished!");
}

fn read_and_print(file_path: &str) -> io::Result<()> {
    let path = Path::new(file_path);

    if !path.exists() {
        std::fs::read_to_string(path).expect("File not found!");
    }

    let content = std::fs::read_to_string(path)?;
    println!("{}", content);

    Ok(())
}

fn file_size(file_path: &str) -> Result<u64, &str> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err("File not found!");
    }

    let metadata = fs::metadata(path).map_err(|_| "Failed to get file metadata!")?;
    Ok(metadata.len() as u64)
}
fn main() {
    count_down(5);

    if let Err(e) = read_and_print("/etc/hosts") {
        eprintln!("Error: {}", e);
    }
    let mut file_path = String::new();
    println!("Enter the file path to check its size:");
    std::io::stdin().read_line(&mut file_path).unwrap();
    match file_size(&file_path.trim()) {
        //trim用于去除输入中的换行符
        Ok(size) => println!("File size: {} bytes", size),
        Err(e) => eprintln!("{}", e),
    }

    print!("{}", "INFO: ".green()); //不换行vs换行
    println!("{}", "Hello, world!".white());
    print!("{}", "WARNING".yellow().bold().underline());
    println!("{}", ": I'm a teapot!".yellow().bold());
    let width = size().unwrap().0 as usize; //获取终端宽度
    println!("{:^width$}", "ERROR: KERNEL PANIC!!!".red().bold());
}

fn humanized_size(size: u64) -> (f64, &'static str) {
    let units = ["B", "KiB", "MiB", "GiB"];
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < units.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    return (size, units[unit_index]);
}

enum Shape {
    Circle{radius: f64},         //半径
    Rectangle{width: f64, height: f64}, //宽和高
}

impl Shape {
    pub fn area(&self) -> f64 {
        match self {
            Shape::Circle{radius} => std::f64::consts::PI * radius * radius,
            Shape::Rectangle{width, height} => width * height,
        }
    }
}
#[derive(Debug,PartialEq,Eq)]
struct UniqueId(u16);

impl UniqueId{
    pub fn new()->Self{
        static ID:AtomicU16=AtomicU16::new(0);
        UniqueId(ID.fetch_add(1, Ordering::Relaxed))
    }
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

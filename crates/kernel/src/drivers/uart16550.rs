use core::fmt;

use bitflags::bitflags;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

const DATA_OFFSET: u16 = 0;
const INTERRUPT_ENABLE_OFFSET: u16 = 1;
const FIFO_CONTROL_OFFSET: u16 = 2;
const LINE_CONTROL_OFFSET: u16 = 3;
const MODEM_CONTROL_OFFSET: u16 = 4;
const LINE_STATUS_OFFSET: u16 = 5;

bitflags! {
    struct LineControlFlags: u8 {
        const DATA_BITS_8 = 0b0000_0011;
        const DLAB = 0b1000_0000;
    }
}

bitflags! {
    struct FifoControlFlags: u8 {
        const ENABLE_FIFO = 0b0000_0001;
        const CLEAR_RECEIVE_FIFO = 0b0000_0010;
        const CLEAR_TRANSMIT_FIFO = 0b0000_0100;
        const TRIGGER_LEVEL_14 = 0b1100_0000;
    }
}

bitflags! {
    struct ModemControlFlags: u8 {
        const DATA_TERMINAL_READY = 0b0000_0001;
        const REQUEST_TO_SEND = 0b0000_0010;
        const OUT1 = 0b0000_0100;
        const OUT2 = 0b0000_1000;
        const LOOPBACK = 0b0001_0000;
    }
}

bitflags! {
    struct LineStatusFlags: u8 {
        const DATA_READY = 0b0000_0001;
        const TRANSMITTER_HOLDING_REGISTER_EMPTY = 0b0010_0000;
    }
}

pub struct SerialPort<const BASE_ADDR: u16> {
    data: Port<u8>,
    interrupt_enable: Port<u8>,
    fifo_control: PortWriteOnly<u8>,
    line_control: Port<u8>,
    modem_control: Port<u8>,
    line_status: PortReadOnly<u8>,
}

impl<const BASE_ADDR: u16> SerialPort<BASE_ADDR> {
    pub const unsafe fn new() -> Self {
        Self {
            data: Port::new(BASE_ADDR + DATA_OFFSET),
            interrupt_enable: Port::new(BASE_ADDR + INTERRUPT_ENABLE_OFFSET),
            fifo_control: PortWriteOnly::new(BASE_ADDR + FIFO_CONTROL_OFFSET),
            line_control: Port::new(BASE_ADDR + LINE_CONTROL_OFFSET),
            modem_control: Port::new(BASE_ADDR + MODEM_CONTROL_OFFSET),
            line_status: PortReadOnly::new(BASE_ADDR + LINE_STATUS_OFFSET),
        }
    }

    pub fn init(&mut self) {
        unsafe {
            // 在配置期间先关闭 UART 的全部中断。
            self.interrupt_enable.write(0x00);

            // 打开 DLAB，使波特率分频寄存器可以被访问。
            self.line_control.write(LineControlFlags::DLAB.bits());

            // 将分频系数设置为 3（低字节 0x03，高字节 0x00），对应 38400 波特率。
            self.data.write(0x03);
            self.interrupt_enable.write(0x00);

            // 关闭 DLAB，并配置为 8 位数据位、无校验、1 位停止位（8N1）。
            self.line_control
                .write(LineControlFlags::DATA_BITS_8.bits());

            // 启用 FIFO，清空收发缓冲区，并设置 14 字节触发阈值。
            self.fifo_control.write(
                (FifoControlFlags::ENABLE_FIFO
                    | FifoControlFlags::CLEAR_RECEIVE_FIFO
                    | FifoControlFlags::CLEAR_TRANSMIT_FIFO
                    | FifoControlFlags::TRIGGER_LEVEL_14)
                    .bits(),
            );

            // 先开启 IRQ、RTS 和 DSR，准备进入正常工作配置。
            self.modem_control.write(
                (ModemControlFlags::DATA_TERMINAL_READY
                    | ModemControlFlags::REQUEST_TO_SEND
                    | ModemControlFlags::OUT2)
                    .bits(),
            );

            // 进入回环模式，准备进行串口芯片自检。
            self.modem_control.write(
                (ModemControlFlags::REQUEST_TO_SEND
                    | ModemControlFlags::OUT1
                    | ModemControlFlags::OUT2
                    | ModemControlFlags::LOOPBACK)
                    .bits(),
            );

            // 发送测试字节并检查是否能够被正确回读。
            self.data.write(0xAE);
            assert_eq!(
                self.data.read(),
                0xAE,
                "UART16550 loopback self-test failed"
            );

            // 退出回环模式，切换回正常工作状态。
            self.modem_control.write(
                (ModemControlFlags::DATA_TERMINAL_READY
                    | ModemControlFlags::REQUEST_TO_SEND
                    | ModemControlFlags::OUT1
                    | ModemControlFlags::OUT2)
                    .bits(),
            );
        }
    }

    pub fn send(&mut self, data: u8) {
        unsafe {
            while !LineStatusFlags::from_bits_retain(self.line_status.read())
                .contains(LineStatusFlags::TRANSMITTER_HOLDING_REGISTER_EMPTY)
            {}
            self.data.write(data);
        }
    }

    pub fn receive(&mut self) -> Option<u8> {
        unsafe {
            if !LineStatusFlags::from_bits_retain(self.line_status.read())
                .contains(LineStatusFlags::DATA_READY)
            {
                None
            } else {
                Some(self.data.read())
            }
        }
    }
}

impl<const BASE_ADDR: u16> fmt::Write for SerialPort<BASE_ADDR> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}

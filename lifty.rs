/*
lifty.rs

Author:  David Beazley (https://www.dabeaz.com)
Source:  https://github.com/dabeaz/lifty

Copyright (C) 2025
All Rights Reserved

This code may be freely copied, modified, and used for EDUCATIONAL
PURPOSES ONLY provided that the above attribution, URLs, and copyright
notice are preserved in all copies.
-----------------------------------------------------------------------------

Hi, I'm a Lifty, a hardware simulator for a basic 5-floor elevator
system with a single elevator car.  I have the following hardware
features:

  - A motor that makes the car go up and down.
  - A door that can open and close.
  - A panel of 5 buttons inside the elevator car.
  - Up request buttons on floors 1-4.
  - Down request buttons on floors 2-5.
  - A direction indicator light on each floor.

Residents of the building interact with me by pressing buttons.
This is done by typing the following commands at the keyboard:

  Pn - Press button for floor n in the elevator car
  Un - Press up button on floor n
  Dn - Press down button on floor n

Sadly, I don't have any brains of my own to know what to do
when a button is pressed.  However, I can interact with a
separate control program via UDP sockets.

     Resident -> [ Lifty ] <--------> [ Control ]
           buttons             UDP

I will send the following event messages to the controller:

  Pn - Panel button for floor n was pressed
  Un - Up button on floor n was pressed
  Dn - Down button floor n was pressed
  An - Approaching floor n (still in motion)
  Sn - Stopped at floor n (safe to open door)
  On - Door open on floor n (doors have fully opened)
  Cn - Door closed on floor n (now safe to move)

I understand the following commands from the controller

  MU  - Start moving up
  MD  - Start moving down
  S   - Stop at the next floor (generates Sn event when stopped)
  DO  - Open door (will generate On event when done)
  DC  - Close the door (will generate Cn event when done)
  CPn - Clear panel button n
  CUn - Clear up button n
  CDn - Clear down button n
  IUn - Set indicator light on floor n to "up"
  IDn - Set indicator light on floor n to "down"
  CIn - Clear the indicator light on floor n
  R   - Reset

Although I don't have any brains, I am programmed with some
some basic protection features and am prone to crashing
if I'm given bad instructions.  If I crash, I'll enter a
permanent crashed state that can only be reset by rebooting
the control software and having it send a reset (R) command.

Your challenge, should you choose to accept it--write a control
program that runs the elevator algorithm and prove that (a) it works
like an actual elevator and (b) it will never cause the elevator to
crash.  Good luck!

*/

// This is a single-file Rust program with no dependencies.
// Compile using `rustc lifty.rs`.

// Network ports for myself and the control program.
const MY_ADDRESS: &str = "127.0.0.1:10000";
const CONTROL_ADDRESS: &str = "127.0.0.1:11000";

// Internal timing
const TICKS_PER_FLOOR: usize = 40;
const TICKS_FOR_DOOR: usize = 20;
const APPROACH_TICKS: usize = 10;
const TICK_INTERVAL: u64 = 100;

// Turn this on if you want Lifty to be super picky or
// if you're looking for ways to deduct grading points.
const PEDANTIC: bool = false;

// Hoist motor status
#[derive(Debug, Clone, PartialEq)]
enum Motor {
    Up,
    Down,
    Off,
}

// Door status
#[derive(Debug, Clone, PartialEq)]
enum Door {
    Opening,
    Open,
    Closing,
    Closed,
}

// Direction indicators
#[derive(Debug, Clone, PartialEq)]
enum Indicator {
    Up,
    Down,
    Off,
}

#[derive(Debug)]
struct Elevator {
    pub floor: usize,
    pub panel_buttons: [bool; 5], // Buttons in the car
    pub up_buttons: [bool; 5],    // Up buttons in the building
    pub down_buttons: [bool; 5],  // Down buttons in the building
    pub indicator: Indicator,     // Indicator light status
    pub indicator_floor: usize,
    pub clock: usize,
    pub motor: Motor,
    pub door: Door,
    pub stopping: bool,
    pub crashed: bool,
}

impl Elevator {
    fn new() -> Elevator {
        Elevator {
            floor: 1,
            panel_buttons: [false, false, false, false, false],
            up_buttons: [false, false, false, false, false],
            down_buttons: [false, false, false, false, false],
            indicator: Indicator::Off,
            indicator_floor: 1,
            clock: 0,
            motor: Motor::Off,
            door: Door::Closed,
            stopping: false,
            crashed: false,
        }
    }

    fn reset(&mut self) {
        self.floor = 1;
        self.panel_buttons = [false, false, false, false, false];
        self.up_buttons = [false, false, false, false, false];
        self.down_buttons = [false, false, false, false, false];
        self.indicator = Indicator::Off;
        self.indicator_floor = 1;
        self.clock = 0;
        self.motor = Motor::Off;
        self.door = Door::Closed;
        self.stopping = false;
        self.crashed = false;
    }

    fn crash(&mut self, reason: &str) {
        println!("\nCRASH! : {reason}");
        self.crashed = true;
    }

    fn as_string(&self) -> String {
        let mut ps = String::from("P:");
        for (n, floor) in self.panel_buttons.iter().enumerate() {
            if *floor {
                ps.push(char::from_u32(49 + n as u32).unwrap());
            } else {
                ps.push('-');
            }
        }
        let mut us = String::from("U:");
        for (n, floor) in self.up_buttons.iter().enumerate() {
            if *floor {
                us.push(char::from_u32(49 + n as u32).unwrap());
            } else {
                us.push('-');
            }
        }
        let mut ds = String::from("D:");
        for (n, floor) in self.down_buttons.iter().enumerate() {
            if *floor {
                ds.push(char::from_u32(49 + n as u32).unwrap());
            } else {
                ds.push('-');
            }
        }
        let indicator = if self.indicator_floor == self.floor {
            match self.indicator {
                Indicator::Up => "^^",
                Indicator::Down => "vv",
                Indicator::Off => "--",
            }
        } else {
            "--"
        };
        let status = if self.crashed {
            "CRASH"
        } else if self.stopping && self.clock >= (TICKS_PER_FLOOR - APPROACH_TICKS) {
            "STOPPING"
        } else if self.motor == Motor::Up {
            "UP"
        } else if self.motor == Motor::Down {
            "DOWN"
        } else if self.door == Door::Opening {
            "OPENING"
        } else if self.door == Door::Open {
            "OPEN"
        } else if self.door == Door::Closing {
            "CLOSING"
        } else if self.door == Door::Closed {
            "CLOSED"
        } else {
            panic!("Can't determine status")
        };
        format!(
            "[ FLOOR {} | {status:8} {indicator} | {ps} | {us} | {ds} ]",
            self.floor
        )
    }

    fn set_panel_button(&mut self, floor: usize) {
        self.panel_buttons[floor - 1] = true;
    }

    fn clear_panel_button(&mut self, floor: usize) {
        if PEDANTIC && !self.panel_buttons[floor - 1] {
            self.crash("panel button not previously set");
        } else {
            self.panel_buttons[floor - 1] = false;
        }
    }

    fn set_up_button(&mut self, floor: usize) {
        self.up_buttons[floor - 1] = true;
    }

    fn clear_up_button(&mut self, floor: usize) {
        if PEDANTIC && !self.up_buttons[floor - 1] {
            self.crash("up button not previously set");
        } else {
            self.up_buttons[floor - 1] = false;
        }
    }

    fn set_down_button(&mut self, floor: usize) {
        self.down_buttons[floor - 1] = true;
    }

    fn clear_down_button(&mut self, floor: usize) {
        if PEDANTIC && !self.down_buttons[floor - 1] {
            self.crash("down button not previously set");
        } else {
            self.down_buttons[floor - 1] = false;
        }
    }

    fn set_indicator(&mut self, floor: usize, status: Indicator) {
        if self.indicator != Indicator::Off && status != Indicator::Off {
            self.crash("direction indicator already illuminated");
        } else if PEDANTIC && self.indicator == Indicator::Off && status == Indicator::Off {
            self.crash("direction indicator already off");
        } else {
            self.indicator = status;
            self.indicator_floor = floor;
        }
    }

    fn set_motor(&mut self, status: Motor) {
        if self.door != Door::Closed {
            self.crash("motor command received while doors open");
            return;
        }
        if self.motor == Motor::Up && status == Motor::Down {
            self.crash("violent direction switch (up->down)");
            return;
        }
        if self.motor == Motor::Down && status == Motor::Up {
            self.crash("violent direction switch (down->up)");
            return;
        }
        if self.motor != status {
            self.motor = status;
            self.clock = 0;
        } else if status == Motor::Up {
            self.crash("already moving up");
        } else if status == Motor::Down {
            self.crash("already moving down");
        }
    }

    fn set_door(&mut self, status: Door) {
        if self.motor != Motor::Off {
            self.crash("door command received while moving");
            return;
        }
        if self.door == Door::Closing && status != Door::Closed {
            self.crash("door command received while closing");
            return;
        }
        if self.door == Door::Opening && status != Door::Open {
            self.crash("door command received while opening");
            return;
        }
        if self.door == Door::Open && status == Door::Opening {
            self.crash("door already open");
            return;
        }
        if self.door == Door::Closed && status == Door::Closing {
            self.crash("door already closed");
            return;
        }
        self.door = status;
        self.clock = 0;
    }

    fn handle_command(&mut self, cmd: &str) -> Option<String> {
        if cmd == "R" {
            self.reset();
            return None;
        }
        if self.crashed {
            return None;
        }
        match cmd {
            // Button presses
            "P1" | "P2" | "P3" | "P4" | "P5" => {
                self.set_panel_button(cmd[1..].parse().unwrap());
                Some(cmd.to_string())
            }
            "U1" | "U2" | "U3" | "U4" => {
                self.set_up_button(cmd[1..].parse().unwrap());
                Some(cmd.to_string())
            }
            "U5" | "CU5" => {
                self.crash("No up button on top floor");
                None
            }
            "D2" | "D3" | "D4" | "D5" => {
                self.set_down_button(cmd[1..].parse().unwrap());
                Some(cmd.to_string())
            }
            "D1" | "CD1" => {
                self.crash("No down button on bottom floor");
                None
            }
            // Clear buttons
            "CP1" | "CP2" | "CP3" | "CP4" | "CP5" => {
                self.clear_panel_button(cmd[2..].parse().unwrap());
                None
            }
            "CU1" | "CU2" | "CU3" | "CU4" => {
                self.clear_up_button(cmd[2..].parse().unwrap());
                None
            }
            "CD2" | "CD3" | "CD4" | "CD5" => {
                self.clear_down_button(cmd[2..].parse().unwrap());
                None
            }
            // Direction indicator lights
            "IU1" | "IU2" | "IU3" | "IU4" => {
                self.set_indicator(cmd[2..].parse().unwrap(), Indicator::Up);
                None
            }
            "IU5" => {
                self.crash("No up indicator light on top floor");
                None
            }
            "ID2" | "ID3" | "ID4" | "ID5" => {
                self.set_indicator(cmd[2..].parse().unwrap(), Indicator::Down);
                None
            }
            "ID1" => {
                self.crash("No down indicator light on bottom floor");
                None
            }
            "CI1" | "CI2" | "CI3" | "CI4" | "CI5" => {
                self.set_indicator(cmd[2..].parse().unwrap(), Indicator::Off);
                None
            }
            // Motor (from control)
            "MU" => {
                self.set_motor(Motor::Up);
                None
            }
            "MD" => {
                self.set_motor(Motor::Down);
                None
            }
            "S" => {
                if self.stopping {
                    self.crash("Already made a request to stop");
                } else if self.motor != Motor::Off {
                    // If we can safely stop we will.
                    if self.clock <= TICKS_PER_FLOOR - APPROACH_TICKS {
                        self.stopping = true;
                    }
                } else {
                    self.crash("Request to stop, but not moving");
                }
                None
            }
            // Door commands (from control)
            "DO" => {
                self.set_door(Door::Opening);
                None
            }
            "DC" => {
                self.set_door(Door::Closing);
                None
            }
            // Clock
            "T" => self.handle_tick(),
            _ => {
                self.crash("Unrecognized command");
                None
            }
        }
    }

    fn handle_tick(&mut self) -> Option<String> {
        self.clock += 1;
        if self.motor == Motor::Up {
            if self.floor >= 5 {
                self.crash("Hit the roof!");
            } else if self.clock == (TICKS_PER_FLOOR - APPROACH_TICKS) {
                return Some(format!("A{}", self.floor + 1));
            } else if self.clock >= TICKS_PER_FLOOR {
                self.floor += 1;
                self.clock = 0;
                if self.stopping {
                    self.set_motor(Motor::Off);
                    self.stopping = false;
                    return Some(format!("S{}", self.floor));
                }
            }
        } else if self.motor == Motor::Down {
            if self.floor <= 1 {
                self.crash("Hit the ground!");
            } else if self.clock == (TICKS_PER_FLOOR - APPROACH_TICKS) {
                return Some(format!("A{}", self.floor - 1));
            } else if self.clock >= TICKS_PER_FLOOR {
                self.floor -= 1;
                self.clock = 0;
                if self.stopping {
                    self.set_motor(Motor::Off);
                    self.stopping = false;
                    return Some(format!("S{}", self.floor));
                }
            }
        } else if self.door == Door::Closing {
            if self.clock > TICKS_FOR_DOOR {
                self.set_door(Door::Closed);
                return Some(format!("C{}", self.floor));
            }
        } else if self.door == Door::Opening {
            if self.clock > TICKS_FOR_DOOR {
                self.set_door(Door::Open);
                return Some(format!("O{}", self.floor));
            }
        }
        None
    }
}

// Runtime environment for the simulator

use std::io;
use std::io::Write;
use std::net::UdpSocket;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::{thread, time};

enum Command {
    UserInput(String),
    Internal(String),
}

fn read_stdin(tx: Sender<Command>) -> ! {
    loop {
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer).unwrap();
        let cmd = buffer.trim().to_uppercase();
        tx.send(Command::UserInput(cmd)).unwrap();
    }
}

fn generate_clock_ticks(tx: Sender<Command>) -> ! {
    loop {
        thread::sleep(time::Duration::from_millis(TICK_INTERVAL));
        tx.send(Command::Internal(String::from("T"))).unwrap();
    }
}

fn read_socket(address: &str, tx: Sender<Command>) -> ! {
    let socket = UdpSocket::bind(address).unwrap();
    loop {
        let mut buf = [0; 2000];
        match socket.recv_from(&mut buf) {
            Ok((n, _)) => {
                let cmds = String::from_utf8((&buf[0..n]).to_vec()).unwrap();
                for cmd in cmds.lines() {
                    tx.send(Command::Internal(cmd.to_string())).unwrap();
                }
            }
            Err(e) => panic!("IO Error: {}", e),
        }
    }
}

fn spawn_threads() -> Receiver<Command> {
    let (tx, rx) = mpsc::channel::<Command>();
    let itx = tx.clone();
    thread::spawn(move || read_stdin(itx));
    let ttx = tx.clone();
    thread::spawn(move || generate_clock_ticks(ttx));
    thread::spawn(move || read_socket(MY_ADDRESS, tx));
    rx
}

fn main() {
    let mut elev = Elevator::new();
    let command_channel = spawn_threads();
    let mut last = String::new();
    let out_socket = UdpSocket::bind("0.0.0.0:0").unwrap();

    println!("Welcome!  I'm Lifty--a simulated elevator in a 5-floor building.\n");
    println!("I'm just hardware, but you can press my buttons\n(type below and hit return):\n");
    println!("    Pn  - Floor n button on panel inside car");
    println!("    Un  - Up button on floor n");
    println!("    Dn  - Down button on floor n\n");
    println!("If something goes wrong, I'll crash and you'll have to call");
    println!("maintenance to restart the elevator control program.\n");

    let mut print_newline = false;
    loop {
        let es = elev.as_string();
        if es != last {
            if print_newline {
                print!("\n");
            }
            print!("{} : ", es);
            std::io::stdout().flush().unwrap();
            last = es;
        }
        match command_channel.recv() {
            Ok(recvcmd) => {
                let cmd = match recvcmd {
                    Command::UserInput(cmd) => {
                        print_newline = false;
                        last = String::from("");
                        cmd
                    }
                    Command::Internal(cmd) => {
                        if cmd != "T" {
                            print_newline = false;
                            println!("recv: {cmd}");
                            last = String::from("");
                        } else {
                            print_newline = true;
                        }
                        cmd
                    }
                };
                if cmd.len() > 0 {
                    if let Some(outcmd) = elev.handle_command(&cmd) {
                        out_socket
                            .send_to(outcmd.as_bytes(), CONTROL_ADDRESS)
                            .expect("couldn't send data");
                    }
                }
            }
            Err(e) => {
                println!("{:?}", e);
                break;
            }
        };
    }
}

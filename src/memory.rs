// Copyright (C) 2014 The 6502-rs Developers
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
// 1. Redistributions of source code must retain the above copyright
//    notice, this list of conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright
//    notice, this list of conditions and the following disclaimer in the
//    documentation and/or other materials provided with the distribution.
// 3. Neither the names of the copyright holders nor the names of any
//    contributors may be used to endorse or promote products derived from this
//    software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

// JAM: We can probably come up with a better way to represent address ranges.
//      Address range type?
//
// // Address range -- inclusive on both sides
// pub struct AddressRangeIncl {
//     begin: Address,
//     end: Address,
// }

const ADDR_LO_BARE: u16 = 0x0000;
const ADDR_HI_BARE: u16 = 0xFFFF;

pub const MEMORY_ADDRESS_LO: u16 = ADDR_LO_BARE;
pub const MEMORY_ADDRESS_HI: u16 = ADDR_HI_BARE;
pub const STACK_ADDRESS_LO: u16 = 0x0100;
pub const STACK_ADDRESS_HI: u16 = 0x01FF;
pub const NMI_INTERRUPT_VECTOR_LO: u16 = 0xFFFA;
pub const NMI_INTERRUPT_VECTOR_HI: u16 = 0xFFFB;
pub const RESET_VECTOR_LO: u16 = 0xFFFC;
pub const RESET_VECTOR_HI: u16 = 0xFFFD;
pub const IRQ_INTERRUPT_VECTOR_LO: u16 = 0xFFFE;
pub const IRQ_INTERRUPT_VECTOR_HI: u16 = 0xFFFF;

const MEMORY_SIZE: usize = (ADDR_HI_BARE - ADDR_LO_BARE) as usize + 1usize;

/// 64KB memory implementation
#[derive(Copy, Clone, Debug)]
pub struct Memory {
    #[allow(clippy::large_stack_arrays)]
    bytes: [u8; MEMORY_SIZE],
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

/// Control signals driven by the CPU on pins that are asserted alongside bus
/// cycles.
///
/// Signal polarity matches the logical meaning, not the physical pin level:
/// `true` means the signal is active regardless of whether the physical pin is
/// active-high or active-low on real hardware.
///
/// These are passed to [`Bus::get_byte_with_signals`] and
/// [`Bus::set_byte_with_signals`], which are only called by the CPU when at
/// least one signal is asserted. Ordinary bus cycles use [`Bus::get_byte`] and
/// [`Bus::set_byte`] directly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlSignals {
    /// High during the opcode-fetch bus cycle (SYNC pin).
    pub sync: bool,
    /// Asserted during the modify and write cycles of read-modify-write
    /// instructions: ASL, DEC, INC, LSR, ROL, ROR, TRB, TSB (MLB pin,
    /// active low on real hardware).
    pub memory_lock: bool,
    /// Asserted while the CPU reads an interrupt vector (VPB pin, active low
    /// on real hardware).
    pub vector_pull: bool,
}

/// Trait for a bus that can read and write bytes.
///
/// This is used to abstract the memory and I/O operations of the CPU.
///
/// # Examples
///
/// ```
/// use mos6502::memory::{Bus, Memory};
///
/// let mut memory = Memory::new();
/// memory.set_byte(0x0000, 0x12);
/// assert_eq!(memory.get_byte(0x0000), 0x12);
/// ```
pub trait Bus {
    /// Returns the byte at the given address.
    fn get_byte(&mut self, address: u16) -> u8;

    /// Sets the byte at the given address to the given value.
    fn set_byte(&mut self, address: u16, value: u8);

    /// Returns the byte at the given address with CPU control signals.
    ///
    /// Called by the CPU only when at least one control signal is asserted
    /// (SYNC during opcode fetch, memory_lock during RMW writes, vector_pull
    /// during interrupt vector reads). Ordinary bus cycles use
    /// [`get_byte`](Bus::get_byte) directly.
    ///
    /// The default implementation ignores `signals` and delegates to
    /// [`get_byte`](Bus::get_byte). Override this method to react to CPU
    /// control signals.
    fn get_byte_with_signals(&mut self, address: u16, _signals: ControlSignals) -> u8 {
        self.get_byte(address)
    }

    /// Sets the byte at the given address to the given value with CPU control
    /// signals.
    ///
    /// Called by the CPU only when at least one control signal is asserted
    /// (memory_lock during RMW writes, vector_pull during interrupt vector
    /// reads). Ordinary bus cycles use [`set_byte`](Bus::set_byte) directly.
    ///
    /// The default implementation ignores `signals` and delegates to
    /// [`set_byte`](Bus::set_byte). Override this method to react to CPU
    /// control signals.
    fn set_byte_with_signals(&mut self, address: u16, value: u8, _signals: ControlSignals) {
        self.set_byte(address, value);
    }

    /// Sets a 16-bit word at the given address (little-endian).
    ///
    /// This is a convenience method that sets the low byte at `address`
    /// and the high byte at `address + 1`.
    fn set_word(&mut self, address: u16, value: u16) {
        let bytes = value.to_le_bytes();
        self.set_byte(address, bytes[0]);
        self.set_byte(address.wrapping_add(1), bytes[1]);
    }

    /// Sets the bytes starting at the given address to the given values.
    ///
    /// This is a default implementation that calls `set_byte` for each byte.
    ///
    /// # Note
    ///
    /// This assumes that the length of `values` is less than or equal to
    /// [`u16::MAX`] (65535). If the length of `values` is greater than `u16::MAX`,
    /// this will truncate the length. This assumption is made because the
    /// maximum addressable memory for the 6502 is 64KB.
    #[allow(clippy::cast_possible_truncation)]
    fn set_bytes(&mut self, start: u16, values: &[u8]) {
        for i in 0..values.len() as u16 {
            self.set_byte(start + i, values[i as usize]);
        }
    }

    /// Returns whether an NMI (Non-Maskable Interrupt) is pending.
    ///
    /// NMI is edge-triggered on the falling edge (high → low transition).
    /// The CPU will detect the transition and service the interrupt.
    ///
    /// Implementations may use `&mut self` to acknowledge or clear the pending state.
    ///
    /// Default implementation returns `false` (no NMI pending).
    ///
    /// # References
    ///
    /// - [W65C02S Datasheet, Section 3.6 (NMIB)](https://www.westerndesigncenter.com/wdc/documentation/w65c02s.pdf)
    fn nmi_pending(&mut self) -> bool {
        false
    }

    /// Returns whether an IRQ (Interrupt Request) is pending.
    ///
    /// IRQ is level-triggered and can be masked by the I flag in the status register.
    /// The interrupt will be serviced while pending and interrupts are enabled.
    ///
    /// Implementations may use `&mut self` to acknowledge or clear the pending state.
    ///
    /// Default implementation returns `false` (no IRQ pending).
    ///
    /// # References
    ///
    /// - [W65C02S Datasheet, Section 3.4 (IRQB)](https://www.westerndesigncenter.com/wdc/documentation/w65c02s.pdf)
    fn irq_pending(&mut self) -> bool {
        false
    }
}

impl Memory {
    #[must_use]
    #[allow(clippy::large_stack_arrays)]
    pub const fn new() -> Memory {
        Memory {
            #[allow(clippy::large_stack_arrays)]
            bytes: [0; MEMORY_SIZE],
        }
    }
}

impl Bus for Memory {
    fn get_byte(&mut self, address: u16) -> u8 {
        self.bytes[address as usize]
    }

    /// Sets the byte at the given address to the given value and returns the
    /// previous value at the address.
    fn set_byte(&mut self, address: u16, value: u8) {
        self.bytes[address as usize] = value;
    }

    /// Fast way to set multiple bytes in memory when the underlying memory is a
    /// consecutive block of bytes.
    fn set_bytes(&mut self, start: u16, values: &[u8]) {
        let start = start as usize;

        // This panics if the range is invalid
        let end = start + values.len();

        self.bytes[start..end].copy_from_slice(values);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "range end index 65537 out of range for slice of length 65536")]
    fn test_memory_overflow_panic() {
        let mut memory = Memory::new();
        memory.set_bytes(0xFFFE, &[1, 2, 3]);
    }

    // A minimal Bus that records which ControlSignals were seen on each access.
    struct SignalRecordingBus {
        data: [u8; 0x10000],
        last_read_signals: Option<ControlSignals>,
        last_write_signals: Option<ControlSignals>,
    }

    impl SignalRecordingBus {
        fn new() -> Self {
            Self {
                data: [0; 0x10000],
                last_read_signals: None,
                last_write_signals: None,
            }
        }
    }

    impl Bus for SignalRecordingBus {
        fn get_byte(&mut self, address: u16) -> u8 {
            self.data[address as usize]
        }

        fn set_byte(&mut self, address: u16, value: u8) {
            self.data[address as usize] = value;
        }

        fn get_byte_with_signals(&mut self, address: u16, signals: ControlSignals) -> u8 {
            self.last_read_signals = Some(signals);
            self.get_byte(address)
        }

        fn set_byte_with_signals(&mut self, address: u16, value: u8, signals: ControlSignals) {
            self.last_write_signals = Some(signals);
            self.set_byte(address, value);
        }
    }

    #[test]
    fn signals_default_impl_forwards_to_get_set_byte() {
        // Memory's Bus impl does not override _with_signals, so the default
        // forwards to get_byte/set_byte transparently.
        let mut mem = Memory::new();
        mem.set_byte_with_signals(0x10, 0xAB, ControlSignals { sync: true, ..ControlSignals::default() });
        assert_eq!(mem.get_byte_with_signals(0x10, ControlSignals::default()), 0xAB);
    }

    #[test]
    fn signals_delivered_to_overriding_impl() {
        let mut bus = SignalRecordingBus::new();
        bus.set_byte(0x20, 0x42);

        let sync = ControlSignals { sync: true, ..ControlSignals::default() };
        assert_eq!(bus.get_byte_with_signals(0x20, sync), 0x42);
        assert_eq!(bus.last_read_signals, Some(sync));

        let mlb = ControlSignals { memory_lock: true, ..ControlSignals::default() };
        bus.set_byte_with_signals(0x20, 0xFF, mlb);
        assert_eq!(bus.last_write_signals, Some(mlb));
        assert_eq!(bus.get_byte(0x20), 0xFF);
    }
}

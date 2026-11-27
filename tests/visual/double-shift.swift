// Posts two real Shift key taps (as two separate key-down/key-up pairs).
//
// cliclick cannot express "Shift Shift": `kp:shift` is rejected because Shift is
// not a named key in cliclick's key list, and `kd:shift ku:shift` twice in one
// invocation is collapsed into a single modifier press by the event tap.
// OpenIDEA (like IntelliJ IDEA) opens Search Everywhere on a double Shift, and
// it de-duplicates synthetic modifier-only events that arrive too close
// together, so this helper spaces the taps out and keeps the window focused.
//
// Usage:
//   swift tests/visual/double-shift.swift            # opens Search Everywhere
//
// Timing was tuned against OpenIDEA 0.1.54: 30 ms hold, 180 ms gap between taps.
import CoreGraphics
import Foundation

let source = CGEventSource(stateID: .hidSystemState)

func tap(_ down: Bool) {
    guard let event = CGEvent(keyboardEventSource: source, virtualKey: 56, keyDown: down) else {
        return
    }
    event.flags = down ? .maskShift : []
    event.post(tap: .cghidEventTap)
}

tap(true)
usleep(30_000)
tap(false)
usleep(180_000)
tap(true)
usleep(30_000)
tap(false)
usleep(300_000)

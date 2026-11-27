// Reports whether the GUI session is locked.
//
// A locked (or asleep) display cannot present new Metal frames, so a window
// capture of a GPU-rendered app can come back with only the first partial
// frame - shapes without text or icons. That kind of image looks like a real
// capture but is not the running UI, so capture-window.sh refuses to write one
// while this reports "locked".
import CoreGraphics
import Foundation

let session = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
let locked = session["CGSSessionScreenIsLocked"] as? Bool ?? false
print(locked ? "locked" : "unlocked")
exit(locked ? 2 : 0)

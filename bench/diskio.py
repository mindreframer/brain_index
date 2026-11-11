"""Prints bytes written to disk by a process (macOS proc_pid_rusage). Usage: python3 -I diskio.py <pid>"""
import ctypes, sys
class RUsage(ctypes.Structure):
    _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [(f, ctypes.c_uint64) for f in (
        "user_time", "system_time", "pkg_idle_wkups", "interrupt_wkups", "pageins", "wired_size",
        "resident_size", "phys_footprint", "proc_start_abstime", "proc_exit_abstime",
        "child_user_time", "child_system_time", "child_pkg_idle_wkups", "child_interrupt_wkups",
        "child_pageins", "child_elapsed_abstime", "diskio_bytesread", "diskio_byteswritten")]
lib = ctypes.CDLL("/usr/lib/libproc.dylib")
ru = RUsage()
if lib.proc_pid_rusage(int(sys.argv[1]), 2, ctypes.byref(ru)) != 0:
    sys.exit("proc_pid_rusage failed")
print(ru.diskio_byteswritten, ru.resident_size)

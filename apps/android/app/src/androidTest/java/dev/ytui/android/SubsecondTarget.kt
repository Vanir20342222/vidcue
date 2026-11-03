package dev.ytui.android

import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert

/**
 * The 1000 ms tap-to-output target of the subsecond startup work (M9).
 *
 * Hosted CI runs a software-rendered emulator: decoder start and pixel readback alone take
 * hundreds of milliseconds there. On 2026-11-03 the owner decided that this target is recorded
 * as advisory evidence on that emulator instead of blocking CI. Real output is still required:
 * actual RGB pixels and active playback within the startup gate ([StartupBudget]). A missed
 * target is reported, never described as met.
 *
 * Pass `-e ytui_subsecond enforce` (or `YTUI_ANDROID_SUBSECOND=enforce` with
 * `tools/android/run-device-tests.sh`) to make the 1000 ms target a hard assertion again, for
 * example on a physical device.
 */
internal object SubsecondTarget {
    const val TARGET_MS = 1000L

    /**
     * The tap-to-visible-frame startup gate: 2000 ms when timing is enforced (nightly, physical
     * devices), otherwise a hang-catching ceiling. See [StartupBudget].
     */
    val FUNCTIONAL_MS: Long
        get() = StartupBudget.ms()

    val enforced: Boolean
        get() = InstrumentationRegistry.getArguments().getString("ytui_subsecond") == "enforce"

    /** How long a test keeps observing after the tap. */
    val observeMs: Long
        get() = if (enforced) TARGET_MS else FUNCTIONAL_MS

    fun met(rgb: Boolean, visible: Long, usable: Long): Boolean =
        rgb && visible in 1 until TARGET_MS && usable in 1 until TARGET_MS

    /**
     * Fails on missing output, or on a missed target when enforced. Returns the evidence line,
     * which states whether the target was met and in which mode.
     */
    fun check(name: String, rgb: Boolean, visible: Long, usable: Long, report: String): String {
        val met = met(rgb, visible, usable)
        val line =
            "$name subsecond_target=${if (met) "met" else "missed"} " +
                "mode=${if (enforced) "enforced" else "advisory"} " +
                "pixels_ms=$visible usable_ms=$usable"
        Log.i("SubsecondTarget", line)
        if (enforced)
            Assert.assertTrue(
                "$name actual visible and playing output must be under${TARGET_MS}ms: " +
                    "pixels=$visible usable=$usable\n$report",
                met,
            )
        else
            Assert.assertTrue(
                "$name actual visible and playing output must arrive within ${FUNCTIONAL_MS}ms: " +
                    "pixels=$visible usable=$usable\n$report",
                rgb && visible in 1..FUNCTIONAL_MS && usable in 1..FUNCTIONAL_MS,
            )
        return line
    }
}

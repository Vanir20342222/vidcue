package dev.ytui.android

import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sqrt

/**
 * Colour grading math shared by the video player (no Android types, so it is JVM-testable).
 * Values use mpv's -100..100 equalizer scale and the same looks and auto-grade rules as the
 * desktop player (crates/vidcue-services/src/color_grade.rs).
 */
object ColorGradeMath {
    /** `color_grade` choices; the first is the default. */
    val PRESETS = listOf("off", "warm", "cool", "vivid", "cinematic")
    private const val LIMIT = 30.0

    data class Grade(
        val brightness: Double = 0.0,
        val contrast: Double = 0.0,
        val saturation: Double = 0.0,
        val gamma: Double = 0.0,
        val temperature: Double = 0.0,
    ) {
        operator fun plus(o: Grade) =
            Grade(
                brightness + o.brightness,
                contrast + o.contrast,
                saturation + o.saturation,
                gamma + o.gamma,
                temperature + o.temperature,
            )

        fun blend(target: Grade, weight: Double) =
            Grade(
                brightness + (target.brightness - brightness) * weight,
                contrast + (target.contrast - contrast) * weight,
                saturation + (target.saturation - saturation) * weight,
                gamma + (target.gamma - gamma) * weight,
                temperature + (target.temperature - temperature) * weight,
            )

        /** Whole steps within the overall limit, so tiny changes do not re-apply. */
        fun rounded(): Grade {
            fun r(v: Double) = v.coerceIn(-LIMIT, LIMIT).roundToInt().toDouble()
            return Grade(r(brightness), r(contrast), r(saturation), r(gamma), r(temperature))
        }

        val neutral get() = this == Grade()
    }

    fun preset(name: String): Grade =
        when (name) {
            "warm" -> Grade(saturation = 3.0, temperature = 10.0)
            "cool" -> Grade(contrast = 2.0, temperature = -10.0)
            "vivid" -> Grade(contrast = 5.0, saturation = 15.0)
            "cinematic" -> Grade(contrast = 8.0, saturation = -10.0, gamma = -3.0, temperature = 4.0)
            else -> Grade()
        }

    /** Picture statistics: luma 0..1, chroma about -1..1 (desktop `signalstats` scale). */
    data class Stats(
        val mean: Double,
        val low: Double,
        val high: Double,
        val saturation: Double,
        val cast: Double,
    )

    /**
     * Statistics of a small ARGB snapshot (full-range RGB as displayed). Returns null for an
     * empty or near-black frame, which says nothing about the grade.
     */
    fun stats(pixels: IntArray): Stats? {
        if (pixels.isEmpty()) return null
        val luma = DoubleArray(pixels.size)
        var saturation = 0.0
        var cast = 0.0
        for ((i, p) in pixels.withIndex()) {
            val r = (p shr 16 and 0xff) / 255.0
            val g = (p shr 8 and 0xff) / 255.0
            val b = (p and 0xff) / 255.0
            luma[i] = 0.2126 * r + 0.7152 * g + 0.0722 * b
            val cb = -0.1146 * r - 0.3854 * g + 0.5 * b
            val cr = 0.5 * r - 0.4542 * g - 0.0458 * b
            // Limited-range chroma spans ±112 code values; desktop divides by 128.
            saturation += sqrt(cb * cb + cr * cr) * 224.0 / 128.0
            cast += (cr - cb) * 224.0 / 128.0
        }
        luma.sort()
        val n = pixels.size
        val s =
            Stats(
                mean = luma.average(),
                low = luma[(n * 0.1).toInt().coerceAtMost(n - 1)],
                high = luma[(n * 0.9).toInt().coerceAtMost(n - 1)],
                saturation = saturation / n,
                cast = cast / n,
            )
        return if (s.high >= 0.06) s else null
    }

    fun average(samples: List<Stats>) =
        Stats(
            samples.map { it.mean }.average(),
            samples.map { it.low }.average(),
            samples.map { it.high }.average(),
            samples.map { it.saturation }.average(),
            samples.map { it.cast }.average(),
        )

    /** A slight correction towards balanced exposure, contrast, saturation and white balance. */
    fun auto(s: Stats) =
        Grade(
            brightness = (-(s.low - 0.15) * 20.0).coerceIn(-4.0, 0.0),
            contrast = ((0.6 - (s.high - s.low)) * 25.0).coerceIn(-3.0, 8.0),
            saturation = ((0.2 - s.saturation) * 50.0).coerceIn(-5.0, 8.0),
            gamma = ((0.42 - s.mean) * 30.0).coerceIn(-5.0, 8.0),
            temperature = (-s.cast * 80.0).coerceIn(-6.0, 6.0),
        )

    /**
     * A 4x5 Android `ColorMatrix` array (offsets in 0..255) for [grade]. Gamma is not linear,
     * so it is approximated by its tangent at mid-grey: midtones move as with mpv's gamma,
     * with a small change at black and white.
     */
    fun matrix(grade: Grade): FloatArray {
        val g = grade.rounded()
        var m = identity()
        // Warmth: red up, blue down.
        val t = g.temperature * 0.004
        m = concat(scale(1 + t, 1.0, 1 - t, 0.0), m)
        // Gamma tangent at 0.5: mpv uses pow(x, 1/8^(gamma/100)).
        val exponent = 1.0 / 8.0.pow(g.gamma / 100.0)
        val slope = exponent * 0.5.pow(exponent - 1)
        val mid = 0.5.pow(exponent)
        m = concat(scale(slope, slope, slope, (mid - slope * 0.5) * 255), m)
        // Contrast around mid-grey, then brightness.
        val k = (g.contrast + 100) / 100
        m = concat(scale(k, k, k, (0.5 - 0.5 * k) * 255 + g.brightness / 100 * 255), m)
        m = concat(saturation((g.saturation + 100) / 100), m)
        return FloatArray(20) { m[it].toFloat() }
    }

    private fun identity() = scale(1.0, 1.0, 1.0, 0.0)

    private fun scale(r: Double, g: Double, b: Double, offset: Double) =
        doubleArrayOf(
            r, 0.0, 0.0, 0.0, offset,
            0.0, g, 0.0, 0.0, offset,
            0.0, 0.0, b, 0.0, offset,
            0.0, 0.0, 0.0, 1.0, 0.0,
        )

    private fun saturation(s: Double): DoubleArray {
        val (lr, lg, lb) = Triple(0.2126 * (1 - s), 0.7152 * (1 - s), 0.0722 * (1 - s))
        return doubleArrayOf(
            lr + s, lg, lb, 0.0, 0.0,
            lr, lg + s, lb, 0.0, 0.0,
            lr, lg, lb + s, 0.0, 0.0,
            0.0, 0.0, 0.0, 1.0, 0.0,
        )
    }

    /** a after b: out = a(b(x)). */
    private fun concat(a: DoubleArray, b: DoubleArray): DoubleArray {
        val out = DoubleArray(20)
        for (row in 0 until 4) {
            for (col in 0 until 5) {
                var v = if (col == 4) a[row * 5 + 4] else 0.0
                for (i in 0 until 4) v += a[row * 5 + i] * b[i * 5 + col]
                out[row * 5 + col] = v
            }
        }
        return out
    }
}

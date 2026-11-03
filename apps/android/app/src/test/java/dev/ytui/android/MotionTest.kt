package dev.ytui.android

import androidx.compose.animation.core.SnapSpec
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.TweenSpec
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class MotionTest {
    @Test
    fun speedScalesDurationsInverselyAndIsClamped() {
        assertEquals(300, Motion(speed = 100).millis(300))
        assertEquals(150, Motion(speed = 200).millis(300))
        assertEquals(600, Motion(speed = 50).millis(300))
        // Out-of-range stored values cannot make animations instant or endless.
        assertEquals(150, Motion(speed = 1000).millis(300))
        assertEquals(600, Motion(speed = 1).millis(300))
    }

    @Test
    fun stylesChangeDurationButKeepTheirOrder() {
        val snappy = Motion(style = "snappy").millis(300)
        val smooth = Motion(style = "smooth").millis(300)
        val gentle = Motion(style = "gentle").millis(300)
        assertTrue(snappy < smooth && smooth < gentle)
    }

    @Test
    fun reducedMotionSnapsEverySpec() {
        val reduced = Motion(style = "bouncy", speed = 50, reduced = true)
        assertEquals(0, reduced.millis(400))
        assertTrue(reduced.enter<Float>(300) is SnapSpec)
        assertTrue(reduced.exit<Float>(300) is SnapSpec)
        assertTrue(reduced.fade<Float>(300) is SnapSpec)
        assertTrue(reduced.spring<Float>() is SnapSpec)
    }

    @Test
    fun springsGetStifferWhenFasterAndBounceOnlyWhenBouncy() {
        val slow = Motion(speed = 50).spring<Float>() as SpringSpec
        val fast = Motion(speed = 200).spring<Float>() as SpringSpec
        assertTrue(fast.stiffness > slow.stiffness)
        assertEquals(1f, (Motion(style = "snappy").spring<Float>() as SpringSpec).dampingRatio)
        assertTrue((Motion(style = "bouncy").spring<Float>() as SpringSpec).dampingRatio < 1f)
    }

    @Test
    fun entranceDelaysScaleWithSpeed() {
        val spec = Motion(speed = 200).enter<Float>(300, delay = 100) as TweenSpec
        assertEquals(150, spec.durationMillis)
        assertEquals(50, spec.delay)
    }

    @Test
    fun dragSettlingSpringsAreCrispAndOrderedByStyle() {
        fun stiffness(style: String) = (Motion(style = style).spring<Float>() as SpringSpec).stiffness
        assertTrue(stiffness("snappy") > stiffness("smooth"))
        assertTrue(stiffness("smooth") > stiffness("gentle"))
        // Release springs must not be the slow "medium-low" default that felt laggy.
        assertTrue(stiffness("smooth") >= 1000f)
    }
}

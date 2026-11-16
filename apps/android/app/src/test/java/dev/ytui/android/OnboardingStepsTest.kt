package dev.ytui.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OnboardingStepsTest {
    @Test
    fun navigationIsClampedToTheThreePages() {
        assertEquals(3, OnboardingSteps.COUNT)
        assertEquals(1, OnboardingSteps.next(0))
        assertEquals(2, OnboardingSteps.next(2))
        assertEquals(0, OnboardingSteps.previous(0))
        assertEquals(1, OnboardingSteps.previous(2))
        assertEquals(0, OnboardingSteps.clamp(-7))
        assertEquals(2, OnboardingSteps.clamp(99))
    }

    @Test
    fun pagesAreWelcomeLookAndAccount() {
        assertEquals(0, OnboardingSteps.WELCOME)
        assertEquals(1, OnboardingSteps.LOOK)
        assertEquals(2, OnboardingSteps.ACCOUNT)
        assertTrue(OnboardingSteps.isLast(OnboardingSteps.ACCOUNT))
    }

    @Test
    fun backLeavesSetupOnlyFromTheFirstPage() {
        assertNull(OnboardingSteps.backTarget(0))
        assertEquals(0, OnboardingSteps.backTarget(1))
        assertEquals(1, OnboardingSteps.backTarget(2))
    }

    @Test
    fun skipIsAvailableOnEveryPageExceptTheFinishPage() {
        for (step in 0..1) assertTrue(OnboardingSteps.showSkip(step))
        assertFalse(OnboardingSteps.showSkip(2))
        assertTrue(OnboardingSteps.isLast(2))
        assertFalse(OnboardingSteps.isLast(1))
    }

    @Test
    fun directionAndDescriptionFollowTheStep() {
        assertTrue(OnboardingSteps.isForward(1, 2))
        assertFalse(OnboardingSteps.isForward(2, 1))
        assertEquals("Step 1 of 3", OnboardingSteps.stateDescription(0))
        assertEquals("Step 3 of 3", OnboardingSteps.stateDescription(2))
    }

    @Test
    fun swipesNeedAFullThresholdAndRespectLayoutDirection() {
        assertEquals(2, OnboardingSteps.swipeTarget(1, -120f, 80f))
        assertEquals(0, OnboardingSteps.swipeTarget(1, 120f, 80f))
        assertEquals(1, OnboardingSteps.swipeTarget(1, -79f, 80f))
        assertEquals(0, OnboardingSteps.swipeTarget(0, 300f, 80f))
        assertEquals(2, OnboardingSteps.swipeTarget(2, -300f, 80f))
        assertEquals(0, OnboardingSteps.swipeTarget(1, -120f, 80f, rtl = true))
        assertEquals(2, OnboardingSteps.swipeTarget(1, 120f, 80f, rtl = true))
        assertEquals(1, OnboardingSteps.swipeTarget(1, Float.NaN, 80f))
    }
}

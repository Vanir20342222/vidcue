package dev.ytui.android;

import static org.junit.Assert.*;

import org.junit.Test;

public final class SleepTimerTest {
  @Test
  public void countdownExpiresOnceAtItsDeadline() {
    SleepTimer timer = new SleepTimer();
    assertFalse(timer.active());
    timer.countdown(10_000, 60);
    assertTrue(timer.active());
    assertEquals(70_000, timer.deadline());
    assertEquals(60_000, timer.total());
    assertFalse(timer.pausesAtEndOfTrack());
    assertFalse(timer.expire(69_999));
    // A countdown is not an end-of-track timer: the item ending does not stop it.
    assertFalse(timer.itemEnded());
    assertTrue(timer.expire(70_000));
    assertFalse(timer.active());
    assertEquals(0, timer.deadline());
    assertEquals(0, timer.total());
    assertFalse(timer.expire(80_000));
  }

  @Test
  public void countdownIsAtLeastOneSecond() {
    SleepTimer timer = new SleepTimer();
    timer.countdown(5, 0);
    assertEquals(1_000, timer.total());
    assertEquals(1_005, timer.deadline());
  }

  @Test
  public void endOfTrackWaitsForTheItemEndHoweverLongItTakes() {
    SleepTimer timer = new SleepTimer();
    timer.endOfTrack();
    assertTrue(timer.active());
    assertTrue(timer.pausesAtEndOfTrack());
    assertEquals(0, timer.deadline());
    // No wall-clock deadline: paused hours later, it still waits for the end.
    assertFalse(timer.expire(Long.MAX_VALUE));
    assertTrue(timer.active());
    assertTrue(timer.itemEnded());
    assertFalse(timer.active());
    // The next item plays on.
    assertFalse(timer.itemEnded());
  }

  @Test
  public void choosingOneModeReplacesTheOtherAndCancelClearsBoth() {
    SleepTimer timer = new SleepTimer();
    timer.endOfTrack();
    timer.countdown(0, 900);
    assertFalse(timer.pausesAtEndOfTrack());
    assertEquals(900_000, timer.deadline());
    timer.endOfTrack();
    assertEquals(0, timer.deadline());
    assertEquals(0, timer.total());
    assertTrue(timer.pausesAtEndOfTrack());
    timer.cancel();
    assertFalse(timer.active());
    assertFalse(timer.itemEnded());
    timer.countdown(0, 60);
    timer.cancel();
    assertFalse(timer.active());
    assertFalse(timer.expire(Long.MAX_VALUE));
  }
}

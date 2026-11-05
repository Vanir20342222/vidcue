package dev.ytui.android;

import static org.junit.Assert.*;

import org.junit.Test;

public final class EngineStartupTest {
  @Test
  public void emptyOrMissingResultMeansTheEngineStarted() {
    assertNull(EngineStartup.message(null));
    assertNull(EngineStartup.message(""));
    assertNull(EngineStartup.message("  \n"));
    EngineStartup startup = new EngineStartup();
    startup.record("");
    assertNull(startup.error());
    assertNull(startup.claimNotice());
  }

  @Test
  public void startErrorIsKeptAndAnnouncedOnce() {
    EngineStartup startup = new EngineStartup();
    startup.record("could not open state database:\n  disk is full");
    String expected = "Vidcue engine could not start: could not open state database: disk is full";
    assertEquals(expected, startup.error());
    assertEquals(expected, startup.claimNotice());
    assertNull("The notice is shown once per launch", startup.claimNotice());
    assertEquals("The status keeps the reason", expected, startup.error());
  }

  @Test
  public void longErrorsAreBounded() {
    StringBuilder raw = new StringBuilder();
    for (int i = 0; i < 100; i++) raw.append("reason ");
    String message = EngineStartup.message(raw.toString());
    assertTrue(message.endsWith("…"));
    assertTrue(
        message.length()
            <= "Vidcue engine could not start: ".length() + EngineStartup.LIMIT);
  }
}

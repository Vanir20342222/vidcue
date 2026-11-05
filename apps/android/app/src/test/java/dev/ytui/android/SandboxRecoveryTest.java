package dev.ytui.android;

import static dev.ytui.android.SandboxRecovery.Stage.*;
import static org.junit.Assert.*;

import androidx.javascriptengine.EvaluationFailedException;
import androidx.javascriptengine.MemoryLimitExceededException;
import androidx.javascriptengine.SandboxDeadException;
import androidx.javascriptengine.SandboxUnsupportedException;
import java.util.concurrent.CancellationException;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeoutException;
import org.junit.Test;

public final class SandboxRecoveryTest {
  @Test
  public void failedOrCancelledConnectionIsReplaced() {
    assertTrue(
        SandboxRecovery.needsReconnect(
            CONNECT, new ExecutionException(new SandboxUnsupportedException("no sandbox"))));
    assertTrue(SandboxRecovery.needsReconnect(CONNECT, new CancellationException()));
    assertTrue(
        SandboxRecovery.needsReconnect(
            CONNECT, new ExecutionException(new RuntimeException("bind failed"))));
  }

  @Test
  public void deadSandboxIsReplacedAtEveryStage() {
    for (SandboxRecovery.Stage stage : SandboxRecovery.Stage.values()) {
      assertTrue(SandboxRecovery.needsReconnect(stage, new SandboxDeadException()));
      assertTrue(
          SandboxRecovery.needsReconnect(
              stage, new ExecutionException(new SandboxDeadException("process died"))));
    }
  }

  @Test
  public void closedSandboxUnderCreateIsolateIsReplaced() {
    assertTrue(
        SandboxRecovery.needsReconnect(
            ISOLATE, new IllegalStateException("Cannot create isolates in closed sandbox")));
  }

  @Test
  public void scriptErrorsLimitsAndTimeoutsKeepTheConnection() {
    assertFalse(SandboxRecovery.needsReconnect(CONNECT, new TimeoutException()));
    assertFalse(SandboxRecovery.needsReconnect(CONNECT, new InterruptedException()));
    assertFalse(SandboxRecovery.needsReconnect(EVALUATE, new TimeoutException()));
    assertFalse(
        SandboxRecovery.needsReconnect(
            EVALUATE, new ExecutionException(new EvaluationFailedException("TypeError"))));
    assertFalse(
        SandboxRecovery.needsReconnect(
            EVALUATE, new ExecutionException(new MemoryLimitExceededException("heap"))));
    assertFalse(
        SandboxRecovery.needsReconnect(ISOLATE, new SandboxRecovery.Refusal("Update WebView")));
    assertFalse(
        SandboxRecovery.needsReconnect(EVALUATE, new IllegalStateException("isolate closed")));
    assertFalse(SandboxRecovery.needsReconnect(EVALUATE, null));
  }

  @Test
  public void retriesOnceAndOnlyWithTimeLeft() {
    assertTrue(SandboxRecovery.mayRetry(0, 1));
    assertFalse(SandboxRecovery.mayRetry(0, 0));
    assertFalse(SandboxRecovery.mayRetry(0, -5));
    assertFalse(SandboxRecovery.mayRetry(1, 10_000));
  }

  @Test
  public void messagesAreShortAndCarryNoStackTrace() {
    assertEquals(SandboxRecovery.TIMED_OUT, SandboxRecovery.message(new TimeoutException()));
    assertEquals(
        "JS output limit", SandboxRecovery.message(new SandboxRecovery.Refusal("JS output limit")));
    String dead =
        SandboxRecovery.message(new ExecutionException(new SandboxDeadException("pid 123 died")));
    assertEquals(SandboxRecovery.UNAVAILABLE, dead);
    assertFalse(dead.contains("Main thread"));
    assertFalse(dead.contains("pid 123"));
  }
}

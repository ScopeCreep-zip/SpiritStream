/**
 * Centralized Heartbeat System
 *
 * Single 1-second timer that conditionally runs registered callbacks.
 * This consolidates multiple polling intervals into one efficient loop.
 *
 * Benefits:
 * - Reduced timer overhead (1 timer instead of N)
 * - Coordinated timing for related operations
 * - Easy to pause/resume all polling
 *
 * Usage:
 *   // Register a callback that runs every 5 seconds
 *   const unsubscribe = heartbeat.register('audio-health', 5000, async () => {
 *     await checkAudioHealth();
 *   });
 *
 *   // Later, unsubscribe
 *   unsubscribe();
 */

interface HeartbeatTask {
  id: string;
  interval: number;
  callback: () => void | Promise<void>;
  lastRun: number;
  enabled: boolean;
}

class HeartbeatManager {
  private tasks = new Map<string, HeartbeatTask>();
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private isRunning = false;
  private tickInterval = 1000; // Base tick rate: 1 second

  /**
   * Register a callback to run at a specified interval.
   * @param id Unique identifier for this task
   * @param interval How often to run (in ms)
   * @param callback Function to call
   * @returns Unsubscribe function
   */
  register(
    id: string,
    interval: number,
    callback: () => void | Promise<void>
  ): () => void {
    const task: HeartbeatTask = {
      id,
      interval,
      callback,
      lastRun: 0,
      enabled: true,
    };

    this.tasks.set(id, task);
    this.ensureRunning();

    return () => {
      this.tasks.delete(id);
      if (this.tasks.size === 0) {
        this.stop();
      }
    };
  }

  /**
   * Enable or disable a task by ID
   */
  setEnabled(id: string, enabled: boolean): void {
    const task = this.tasks.get(id);
    if (task) {
      task.enabled = enabled;
    }
  }

  /**
   * Force a task to run immediately
   */
  runNow(id: string): void {
    const task = this.tasks.get(id);
    if (task && task.enabled) {
      task.lastRun = 0; // Reset lastRun to trigger on next tick
    }
  }

  private ensureRunning(): void {
    if (this.isRunning) return;

    this.isRunning = true;
    this.intervalId = setInterval(() => {
      this.tick();
    }, this.tickInterval);
  }

  private stop(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
    this.isRunning = false;
  }

  private tick(): void {
    const now = Date.now();

    for (const task of this.tasks.values()) {
      if (!task.enabled) continue;

      if (now - task.lastRun >= task.interval) {
        task.lastRun = now;
        try {
          const result = task.callback();
          // Handle async callbacks - don't await, just catch errors
          if (result instanceof Promise) {
            result.catch((err) => {
              console.debug(`[Heartbeat] Task '${task.id}' error:`, err);
            });
          }
        } catch (err) {
          console.debug(`[Heartbeat] Task '${task.id}' error:`, err);
        }
      }
    }
  }

  /**
   * Get the number of registered tasks
   */
  getTaskCount(): number {
    return this.tasks.size;
  }

  /**
   * Stop all tasks and clean up
   */
  stopAll(): void {
    this.tasks.clear();
    this.stop();
  }
}

// Singleton instance
export const heartbeat = new HeartbeatManager();

/**
 * Guards against losing unsaved edits when the user switches channel, event or
 * goes back to the landing screen. An editor with a save step (e.g. the timetable)
 * registers itself; the shell asks before navigating away.
 */
export interface UnsavedGuard {
  /** What has unsaved changes, e.g. "Jadwal pertandingan". */
  label: string;
  isDirty: () => boolean;
  /** Saves; resolves true when the save succeeded. */
  save: () => Promise<boolean>;
}

let current: UnsavedGuard | null = null;

/** Register an editor; returns the unregister function (call it on destroy). */
export function registerUnsavedGuard(guard: UnsavedGuard): () => void {
  current = guard;
  return () => {
    if (current === guard) current = null;
  };
}

/** The registered editor if it has unsaved changes, otherwise null. */
export function dirtyGuard(): UnsavedGuard | null {
  return current && current.isDirty() ? current : null;
}

/** Pending navigation waiting for the user's answer in the leave dialog. */
export const leaveRequest = $state<{ guard: UnsavedGuard | null; proceed: (() => void | Promise<void>) | null; saving: boolean }>({
  guard: null,
  proceed: null,
  saving: false,
});

/**
 * Runs `action` right away when nothing is unsaved; otherwise opens the leave
 * dialog and runs it after the user saves or discards.
 */
export function guardedNavigate(action: () => void | Promise<void>): void {
  const guard = dirtyGuard();
  if (!guard) {
    void action();
    return;
  }
  leaveRequest.guard = guard;
  leaveRequest.proceed = action;
}

export function cancelLeave(): void {
  leaveRequest.guard = null;
  leaveRequest.proceed = null;
  leaveRequest.saving = false;
}

export async function saveAndLeave(): Promise<void> {
  const { guard, proceed } = leaveRequest;
  if (!guard || !proceed) return cancelLeave();
  leaveRequest.saving = true;
  const ok = await guard.save();
  leaveRequest.saving = false;
  if (!ok) return; // keep the dialog open; the editor already showed the error
  cancelLeave();
  await proceed();
}

export async function discardAndLeave(): Promise<void> {
  const proceed = leaveRequest.proceed;
  cancelLeave();
  if (proceed) await proceed();
}

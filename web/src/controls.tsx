// The controls, drawn by Preact. Each reads the signals it shows, so a change redraws that control alone, and each acts
// on the viewer only through the actions it is given.

import type { KernelReport } from "./protocol.ts";
import type { Report } from "./report.ts";
import { failure, notice, refusals, selection, status, watched } from "./state.ts";

export interface Actions {
  fit(): void;
  centreOnSelection(): void;
}

function megabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function Status() {
  const { faces, triangles, pending, fallback } = status.value;
  const uploading = pending > 0 ? `, ${megabytes(pending)} to upload` : "";
  return (
    <span class="status">
      {faces} faces, {triangles} triangles{uploading}{fallback ? ", on a software adapter" : ""}
    </span>
  );
}

function Controls({ actions }: { actions: Actions }) {
  return (
    <div class="controls" role="toolbar" aria-label="View">
      <button type="button" onClick={() => actions.fit()} title="Fit the model to the window">Fit</button>
      <button
        type="button"
        onClick={() => actions.centreOnSelection()}
        disabled={selection.value.length === 0}
        title="Centre the view on the selection"
      >
        Centre
      </button>
      <Status />
    </div>
  );
}

function Shown({ shown, kind }: { shown: Report; kind: string }) {
  return (
    <section
      class={`report ${kind}`}
      role="alert"
      data-code={shown.code}
      data-remedy={shown.remedy?.kind}
      data-entities={shown.entities?.join(" ")}
    >
      <p class="code">{shown.code}</p>
      <p class="message">{shown.message}</p>
      {shown.remedy && <p class="remedy">{shown.remedy.text}</p>}
    </section>
  );
}

/** What the kernel refused of the model shown, each by the entity it names first; the rest of the model is drawn. */
function Refusals({ reports }: { reports: readonly KernelReport[] }) {
  return (
    <div class="refusals" role="status" aria-label="Refused">
      {reports.map((each) => (
        <section class="report refusal" data-code={each.code} data-entity={each.entities[0]} data-reason={each.reason}>
          <p class="code">{each.code} {each.entities[0]}</p>
          <p class="message">{each.message}{each.reason ? ` (${each.reason})` : ""}</p>
        </section>
      ))}
    </div>
  );
}

/** Why the model file does not read, as `occam view` names it; the drawing goes on with the last model that did. */
function Watched({ reports }: { reports: readonly KernelReport[] }) {
  return (
    <div class="watched" role="alert" aria-label="The model file">
      {reports.map((each) => (
        <section class="report model" data-code={each.code} data-entity={each.entities[0]}>
          <p class="code">{each.code}</p>
          <p class="message">{each.message}</p>
        </section>
      ))}
    </div>
  );
}

/** The controls over the drawing, or, when the page cannot draw, the report in its place. */
export function App({ actions }: { actions: Actions | null }) {
  const failed = failure.value;
  if (failed) return <Shown shown={failed} kind="failure" />;
  const shown = notice.value;
  const refused = refusals.value;
  const unread = watched.value;
  return (
    <>
      {actions && <Controls actions={actions} />}
      {unread.length > 0 && <Watched reports={unread} />}
      {shown && <Shown shown={shown} kind="notice" />}
      {refused.length > 0 && <Refusals reports={refused} />}
    </>
  );
}

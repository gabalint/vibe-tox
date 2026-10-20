import { describe, expect, it } from "vitest";
import { engineError } from "@/lib/test/ipc";
import {
  gateLabel,
  toCommandError,
  toFormField,
  toUserMessage,
  toValidationErrors,
} from "./errors";

describe("toCommandError", () => {
  it("passes engine errors through", () => {
    const error = engineError("conflict", "Reload and try again.");
    expect(toCommandError(error)).toBe(error);
  });

  it("maps anything else to unavailable without echoing its text", () => {
    const error = toCommandError("Command sign_in not found at /internal/path");
    expect(error.code).toBe("unavailable");
    expect(JSON.stringify(error)).not.toContain("/internal/path");
    expect(toCommandError({ code: "made_up", message: "x" }).code).toBe("unavailable");
  });
});

describe("toUserMessage", () => {
  it("prefers the engine's message and falls back by code", () => {
    expect(toUserMessage(engineError("forbidden", "You cannot release this report."))).toBe(
      "You cannot release this report.",
    );
    expect(toUserMessage(engineError("forbidden", " "))).toBe(
      "You do not have permission for this action.",
    );
  });
});

describe("toValidationErrors", () => {
  it("keys field errors by field name", () => {
    const error = engineError("validation", "Check the fields.", {
      fieldErrors: [{ field: "code", code: "required", message: "Enter the code." }],
    });
    expect(toValidationErrors(error)).toEqual({ code: "Enter the code." });
    expect(toValidationErrors(null)).toEqual({});
  });

  it("names list entries the way form fields are named", () => {
    const error = engineError("validation", "Check the fields.", {
      fieldErrors: [
        { field: "analytes[1].cutoff", code: "invalid", message: "Check the cutoff." },
        { field: "components[0].analyteIds[2]", code: "duplicate", message: "Listed twice." },
      ],
    });
    expect(toValidationErrors(error)).toEqual({
      "analytes.1.cutoff": "Check the cutoff.",
      "components.0.analyteIds.2": "Listed twice.",
    });
  });
});

describe("toFormField", () => {
  it("turns list indexes into dotted segments", () => {
    expect(toFormField("evidence.diagnoses[0].code")).toBe("evidence.diagnoses.0.code");
    expect(toFormField("code")).toBe("code");
  });

  it("maps whole-segment prefixes where the form's root differs, longest first", () => {
    const paths = { evidence: "", "templateUse.name": "templateName", template: "other" };
    expect(toFormField("evidence.diagnoses[0].code", paths)).toBe("diagnoses.0.code");
    expect(toFormField("evidence.medications[2].asOfDate", paths)).toBe("medications.2.asOfDate");
    expect(toFormField("evidence.authority.documentRef", paths)).toBe("authority.documentRef");
    expect(toFormField("templateUse.name", paths)).toBe("templateName");
    // A prefix matches whole segments only.
    expect(toFormField("evidenceRef", paths)).toBe("evidenceRef");
    expect(toFormField("items[1].reflexScopeOrderableIds", paths)).toBe(
      "items.1.reflexScopeOrderableIds",
    );
  });

  it("keys nested engine errors by their form fields", () => {
    const error = engineError("validation", "Check the fields.", {
      fieldErrors: [
        { field: "evidence.diagnoses[1].codeSystem", code: "required", message: "Enter it." },
      ],
    });
    expect(toValidationErrors(error, { evidence: "" })).toEqual({
      "diagnoses.1.codeSystem": "Enter it.",
    });
  });
});

describe("gateLabel", () => {
  it("labels the review gate codes and nothing else", () => {
    for (const code of ["execution_failed", "run_closed", "qc_conflict", "prep_not_completed"]) {
      expect(gateLabel(code)).toBeTruthy();
    }
    expect(gateLabel("made_up")).toBeUndefined();
    expect(gateLabel("constructor")).toBeUndefined();
  });

  it("labels a release gate code that carries its cause after a colon", () => {
    expect(gateLabel("review_stale:qc_rejected")).toBe("Technical review out of date");
    expect(gateLabel("self_release")).toBe("You reviewed results on this report");
    expect(gateLabel("made_up:review_stale")).toBeUndefined();
  });
});

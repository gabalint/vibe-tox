import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CatalogReviewApproved } from "@/generated/CatalogReviewApproved";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow } from "@/lib/test/grid";
import { engineError } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { PREVIEW_ID, preview, REVIEWED, renderCatalogReview } from "./testData";

type Handler = (args: unknown) => unknown;

const APPROVED: CatalogReviewApproved = {
  reviewId: "00000000-0000-7000-8000-00000000f001",
  validityPolicies: 0,
  methods: 2,
  orderables: 3,
  confirmedBlankCutoffs: 2,
};

/** Opens Approve reviewed catalog on the unreviewed report, as an approver. */
async function openReview(approve: Handler, list: Handler = () => [preview(), REVIEWED]) {
  const rendered = renderCatalogReview(["approve_configuration"], {
    list_catalog_previews: list,
    view_catalog_preview_pdf: () => null,
    approve_reviewed_catalog: approve,
  });
  const { user } = rendered;
  await openRow(user, PREVIEW_ID);
  const panel = within(await screen.findByRole("region", { name: PREVIEW_ID }));
  await user.click(panel.getByRole("button", { name: "Approve reviewed catalog" }));
  const dialog = within(screen.getByRole("alertdialog", { name: "Approve reviewed catalog" }));
  return { ...rendered, dialog };
}

async function confirmAllAndApprove(
  user: Awaited<ReturnType<typeof openReview>>["user"],
  dialog: Awaited<ReturnType<typeof openReview>>["dialog"],
) {
  await user.click(dialog.getByRole("checkbox", { name: "Confirm all listed" }));
  await user.type(dialog.getByLabelText(/Your password/), "synthetic-password");
  await user.click(dialog.getByRole("button", { name: "Approve 5 drafts" }));
}

describe("Approve reviewed catalog (proposed REQ-OPS-015)", () => {
  beforeEach(() => {
    mockGridLayout();
  });

  it("states what it approves, confirms each blank cutoff, and shows the counts", async () => {
    const approve = vi.fn(() => APPROVED);
    const list = vi.fn(() => [preview(), REVIEWED]);
    const { user, dialog } = await openReview(approve, list);
    expect(dialog.getByText(/Approves the 5 drafts listed in report/)).toBeTruthy();
    expect(dialog.getByRole("checkbox", { name: "UR-IA version 1: UR-AMP-IA" })).toBeTruthy();
    expect(dialog.getByRole("checkbox", { name: "UR-OPI-MS version 1: UR-CODE" })).toBeTruthy();
    await user.type(
      dialog.getByLabelText("External approval reference (optional)"),
      "  BATCH-REF-21  ",
    );
    await expectNoA11yViolations(document.body);
    await confirmAllAndApprove(user, dialog);
    expect(await screen.findByText("Reviewed catalog approved")).toBeTruthy();
    expect(screen.getByText("Tests and panels approved").nextSibling?.textContent).toBe("3");
    expect(approve).toHaveBeenCalledWith({
      input: {
        previewId: PREVIEW_ID,
        confirmedBlankCutoffs: [
          {
            methodVersionId: "00000000-0000-7000-8000-00000000e001",
            analyteId: "00000000-0000-7000-8000-00000000e101",
          },
          {
            methodVersionId: "00000000-0000-7000-8000-00000000e002",
            analyteId: "00000000-0000-7000-8000-00000000e102",
          },
        ],
        selfApproval: false,
        password: "synthetic-password",
        externalApprovalReference: "BATCH-REF-21",
      },
    });
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(list).toHaveBeenCalledTimes(2);
  });

  it("requires every listed blank cutoff to be confirmed", async () => {
    const approve = vi.fn(() => APPROVED);
    const { user, dialog } = await openReview(approve);
    await user.click(dialog.getByRole("checkbox", { name: "UR-IA version 1: UR-AMP-IA" }));
    await user.type(dialog.getByLabelText(/Your password/), "synthetic-password");
    await user.click(dialog.getByRole("button", { name: "Approve 5 drafts" }));
    expect(dialog.getByText("Confirm each listed blank cutoff as not applicable.")).toBeTruthy();
    expect(approve).not.toHaveBeenCalled();
  });

  it("explains the engine's refusal of unconfirmed blank cutoffs", async () => {
    const { user, dialog } = await openReview(() =>
      Promise.reject(engineError("validation", "confirm exactly the blank cutoffs: UR-IA v1")),
    );
    await confirmAllAndApprove(user, dialog);
    expect(await dialog.findByText(/Confirm exactly the blank cutoffs listed/)).toBeTruthy();
    expect(dialog.getByText("confirm exactly the blank cutoffs: UR-IA v1")).toBeTruthy();
  });

  it("shows a wrong password on the password field", async () => {
    const { user, dialog } = await openReview(() =>
      Promise.reject(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [{ field: "password", code: "incorrect", message: "Incorrect password." }],
        }),
      ),
    );
    await confirmAllAndApprove(user, dialog);
    expect(await dialog.findByText("Incorrect password.")).toBeTruthy();
    expect(dialog.queryByText("Not approved")).toBeNull();
    expect(screen.getByRole("alertdialog")).toBeTruthy();
  });

  it("keeps an optional approval reference editable after field validation refuses it", async () => {
    const { user, dialog } = await openReview(() =>
      Promise.reject(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            {
              field: "externalApprovalReference",
              code: "control_character",
              message: "Remove control characters.",
            },
          ],
        }),
      ),
    );
    await user.type(dialog.getByLabelText("External approval reference (optional)"), "SOURCE-17");
    await confirmAllAndApprove(user, dialog);
    expect(await dialog.findByText("Remove control characters.")).toBeTruthy();
    expect(
      (dialog.getByLabelText("External approval reference (optional)") as HTMLInputElement).value,
    ).toBe("SOURCE-17");
    expect(screen.getByRole("alertdialog")).toBeTruthy();
  });

  it("refuses a reviewer who authored listed drafts, naming them", async () => {
    const { user, dialog } = await openReview(() =>
      Promise.reject(engineError("gate_blocked", "the reviewer authored these drafts: UR-IA v1")),
    );
    await confirmAllAndApprove(user, dialog);
    expect(await dialog.findByText("Not approved")).toBeTruthy();
    expect(
      dialog.getByText(/the laboratory's separation-of-duties policy needs a second person/),
    ).toBeTruthy();
    expect(dialog.getByText("the reviewer authored these drafts: UR-IA v1")).toBeTruthy();
  });

  it("asks a reviewer who wrote drafts to confirm the self-approval, keeping the password", async () => {
    const message =
      "You wrote this configuration. Your laboratory allows you to approve it yourself: confirm that no second person reviewed it.";
    const approve = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [{ field: "selfApproval", code: "self_approval_unconfirmed", message }],
        }),
      )
      .mockReturnValue(APPROVED);
    const { user, dialog } = await openReview(approve);
    await confirmAllAndApprove(user, dialog);
    const checkbox = await dialog.findByRole("checkbox", { name: /no second person reviewed/ });
    expect(dialog.getByText(message)).toBeTruthy();
    expect(dialog.queryByText("Not approved")).toBeNull();
    await user.click(checkbox);
    await user.click(dialog.getByRole("button", { name: "Approve 5 drafts" }));
    expect(await screen.findByText("Reviewed catalog approved")).toBeTruthy();
    expect(approve).toHaveBeenLastCalledWith({
      input: expect.objectContaining({ selfApproval: true, password: "synthetic-password" }),
    });
  });

  it("asks for a new report when the catalog changed", async () => {
    const { user, dialog } = await openReview(() =>
      Promise.reject(
        engineError("conflict", "the catalog changed after the report was generated: UR-IA v1"),
      ),
    );
    await confirmAllAndApprove(user, dialog);
    expect(await dialog.findByText(/Generate a new Catalog Preview/)).toBeTruthy();
    expect(dialog.getByText("corr-test-0001")).toBeTruthy();
  });

  it("says when the report is gone", async () => {
    const { user, dialog } = await openReview(() =>
      Promise.reject(engineError("not_found", "The record was not found.")),
    );
    await confirmAllAndApprove(user, dialog);
    expect(await dialog.findByText(/This report was not found/)).toBeTruthy();
  });
});

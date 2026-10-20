import type { MethodView } from "@/generated/MethodView";
import type { OrderableVersionContent } from "@/generated/OrderableVersionContent";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import { checked, choiceOf } from "@/lib/choices";
import { PURPOSES } from "../labels";

/**
 * One component as typed. The method is chosen first to list its versions; the engine
 * receives only the method version, the purpose and the analytes reported.
 */
export interface ComponentValues {
  methodId: string | null;
  methodVersionId: string | null;
  purpose: string | null;
  analyteIds: string[];
}

export interface OrderableFormValues {
  components: ComponentValues[];
}

export const EMPTY_COMPONENT: ComponentValues = {
  methodId: null,
  methodVersionId: null,
  purpose: null,
  analyteIds: [],
};

/** A version's components as form values; methods are matched by their code. */
export function orderableValues(
  view: OrderableVersionView | null,
  methods: readonly MethodView[],
): OrderableFormValues {
  if (!view) return { components: [EMPTY_COMPONENT] };
  return {
    components: view.components.map((component) => ({
      methodId: methods.find((method) => method.code === component.methodCode)?.id ?? null,
      methodVersionId: component.methodVersionId,
      purpose: component.purpose,
      analyteIds: component.analytes.map((analyte) => analyte.analyteId),
    })),
  };
}

export function toOrderableContent(values: OrderableFormValues): OrderableVersionContent {
  return {
    components: values.components.map((component) => ({
      methodVersionId: checked(component.methodVersionId),
      purpose: checked(choiceOf(PURPOSES, component.purpose)),
      analyteIds: component.analyteIds,
    })),
  };
}

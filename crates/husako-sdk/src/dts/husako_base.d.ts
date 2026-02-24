import { MetadataFragment, ResourceRequirementsFragment } from "husako";

/** Base class for schema builders (intermediate types like Container, PodSpec). */
export class _SchemaBuilder {
  /** Serialize to a plain object, resolving nested builders. */
  _toJSON(): Record<string, any>;
  /** Render to a plain object for use with build(). */
  _render(): Record<string, any>;
}

/** Base class for Kubernetes resource builders. */
export class _ResourceBuilder {
  /** Set metadata using a MetadataFragment. Returns a new builder (copy-on-write). */
  metadata(fragment: MetadataFragment): this;
  /** Set the resource spec. Returns a new builder (copy-on-write). */
  spec(value: Record<string, any>): this;
  /** Set an arbitrary top-level field. Returns a new builder (copy-on-write). */
  set(key: string, value: any): this;
  /** Set container resource requirements. Returns a new builder (copy-on-write). */
  resources(...fragments: ResourceRequirementsFragment[]): this;
}

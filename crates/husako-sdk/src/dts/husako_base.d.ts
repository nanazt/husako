import { MetadataFragment, ResourceRequirementsFragment } from "husako";

/** Base class for Kubernetes resource builders. */
export class _ResourceBuilder {
  /** Set metadata using a MetadataFragment. Returns a new builder (copy-on-write). */
  metadata(fragment: MetadataFragment): this;
  /** Set container resource requirements. Returns a new builder (copy-on-write). */
  resources(...fragments: ResourceRequirementsFragment[]): this;
}

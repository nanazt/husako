/** Fragment representing Kubernetes metadata (name, namespace, labels, annotations). */
export interface MetadataFragment {
  /** Set the resource name. */
  name(value: string): MetadataFragment;
  /** Set the resource namespace. */
  namespace(value: string): MetadataFragment;
  /** Add a label key-value pair. */
  label(key: string, value: string): MetadataFragment;
  /** Add an annotation key-value pair. */
  annotation(key: string, value: string): MetadataFragment;
}

/** Fragment representing a resource list (cpu, memory). */
export interface ResourceListFragment {
  /** Set cpu quantity. Numbers are converted: integers to string, decimals to millicores (e.g. 0.5 -> "500m"). */
  cpu(value: string | number): ResourceListFragment;
  /** Set memory quantity. Numbers get "Gi" suffix (e.g. 2 -> "2Gi"). */
  memory(value: string | number): ResourceListFragment;
}

/** Fragment representing resource requirements (requests and/or limits). */
export interface ResourceRequirementsFragment {
  /** Chain requests onto this fragment. */
  requests(rl: ResourceListFragment): ResourceRequirementsFragment;
  /** Chain limits onto this fragment. */
  limits(rl: ResourceListFragment): ResourceRequirementsFragment;
}

/** Create an empty MetadataFragment. Entry point for metadata chains. */
export function metadata(): MetadataFragment;

/** Create a MetadataFragment with the given name. */
export function name(value: string): MetadataFragment;

/** Create a MetadataFragment with the given namespace. */
export function namespace(value: string): MetadataFragment;

/** Create a MetadataFragment with a single label. */
export function label(key: string, value: string): MetadataFragment;

/** Create a MetadataFragment with a single annotation. */
export function annotation(key: string, value: string): MetadataFragment;

/** Create a ResourceListFragment with the given cpu quantity. */
export function cpu(value: string | number): ResourceListFragment;

/** Create a ResourceListFragment with the given memory quantity. */
export function memory(value: string | number): ResourceListFragment;

/** Wrap a ResourceListFragment as requests. */
export function requests(rl: ResourceListFragment): ResourceRequirementsFragment;

/** Wrap a ResourceListFragment as limits. */
export function limits(rl: ResourceListFragment): ResourceRequirementsFragment;

/** Merge an array of fragments. Last-argument-wins for scalars, deep-merge for labels/annotations. */
export function merge(items: MetadataFragment[]): MetadataFragment;
export function merge(items: ResourceListFragment[]): ResourceListFragment;

/** Submit resources to husako for rendering. Must be called exactly once. */
export function build(input: { _render(): any } | { _render(): any }[]): void;

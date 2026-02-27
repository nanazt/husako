/** Internal interface for chain fragments returned by starter functions. */
export interface _SpecFragment {
  readonly _husakoTag: "SpecFragment";
  name(v: string): _SpecFragment;
  namespace(v: string): _SpecFragment;
  label(k: string, v: string): _SpecFragment;
  annotation(k: string, v: string): _SpecFragment;
  image(v: string): _SpecFragment;
  imagePullPolicy(v: string): _SpecFragment;
  resources(r: _ResourceRequirementsChain): _SpecFragment;
  command(v: string[]): _SpecFragment;
  args(v: string[]): _SpecFragment;
  _toMetadata(): Record<string, any>;
  _toContainer(): Record<string, any>;
}

/** Bare resource list chain (cpu/memory). Returned by cpu() and memory() starters.
 *  Pass to requests() from k8s/core/v1 to create a full ResourceRequirementsChain. */
export interface _ResourceChain {
  readonly _husakoTag: "ResourceChain";
  cpu(v: string | number): _ResourceChain;
  memory(v: string | number): _ResourceChain;
  /** Returns bare { cpu?, memory? } — no requests wrapper. */
  _toJSON(): Record<string, any>;
}

/** Resource requirements chain carrying requests and optional limits.
 *  Returned by requests() from k8s/core/v1. */
export interface _ResourceRequirementsChain {
  readonly _husakoTag: "ResourceRequirementsChain";
  limits(chain: _ResourceChain): _ResourceRequirementsChain;
  _toJSON(): Record<string, any>;
}

/** Create a SpecFragment with optional initial properties.
 *  Used internally by chain starter functions (name(), image(), etc.). */
export function _createSpecFragment(init?: Record<string, any>): _SpecFragment;

/** Create a bare ResourceChain (cpu/memory list, no requests wrapper).
 *  Used internally by cpu() and memory() chain starters from k8s/core/v1. */
export function _createResourceChain(init?: Record<string, any>): _ResourceChain;

/** Create a ResourceRequirementsChain from a bare resource list.
 *  Used internally by requests() from k8s/core/v1. */
export function _createResourceRequirementsChain(reqList: Record<string, any>): _ResourceRequirementsChain;

/** Base class for schema builders (intermediate types like PodSpec, Container). */
export class _SchemaBuilder {
  /** Serialize to a plain object, resolving nested builders. */
  _toJSON(): Record<string, any>;
  /** Render to a plain object for use with build(). */
  _render(): Record<string, any>;
}

/** Base class for Kubernetes resource builders. */
export class _ResourceBuilder {
  /** Render to a plain object for use with husako.build(). */
  _render(): Record<string, any>;
  /** Set metadata using a MetadataChain or SpecFragment. Accepts any chain fragment from starter functions. */
  metadata(chain: any): this;
  /** Set the resource spec. Returns a new builder (copy-on-write). */
  spec(value: Record<string, any>): this;
  /** Set an arbitrary top-level field. Returns a new builder (copy-on-write). */
  set(key: string, value: any): this;
  /** Set container resource requirements. Use requests() from k8s/core/v1. */
  resources(chain: _ResourceRequirementsChain): this;
}

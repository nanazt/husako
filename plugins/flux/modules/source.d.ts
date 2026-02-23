import { _ResourceBuilder } from "husako/_base";

export interface SourceRef {
  kind: string;
  name: string;
  namespace?: string;
}

export interface GitRef {
  branch?: string;
  tag?: string;
  semver?: string;
  commit?: string;
}

export interface OCIRef {
  tag?: string;
  semver?: string;
  digest?: string;
}

export interface GitRepositoryBuilder extends _ResourceBuilder {
  url(url: string): this;
  ref(ref: GitRef): this;
  interval(interval: string): this;
  secretRef(name: string): this;
}

export interface HelmRepositoryBuilder extends _ResourceBuilder {
  url(url: string): this;
  type(type: "default" | "oci"): this;
  interval(interval: string): this;
  secretRef(name: string): this;
}

export interface OCIRepositoryBuilder extends _ResourceBuilder {
  url(url: string): this;
  ref(ref: OCIRef): this;
  interval(interval: string): this;
  secretRef(name: string): this;
}

export function GitRepository(name?: string): GitRepositoryBuilder;
export function HelmRepository(name?: string): HelmRepositoryBuilder;
export function OCIRepository(name?: string): OCIRepositoryBuilder;

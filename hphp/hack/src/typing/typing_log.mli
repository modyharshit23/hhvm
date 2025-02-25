(*
 * Copyright (c) 2016, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the "hack" directory of this source tree.
 *
 *)

val out_channel : Pervasives.out_channel ref

val log_key : string -> unit

val hh_show_env : Pos.t -> Typing_env_types.env -> unit

val hh_show : Pos.t -> Typing_env_types.env -> Typing_defs.locl_ty -> unit

(** Simple type of possible log data *)
type log_structure =
  | Log_head of string * log_structure list
  | Log_type of string * Typing_defs.locl_ty
  | Log_type_i of string * Typing_defs.internal_type

val log_with_level :
  Typing_env_types.env -> string -> int -> (unit -> unit) -> unit

val log_types : Pos.t -> Typing_env_types.env -> log_structure list -> unit

val log_prop :
  int ->
  Pos.t ->
  string ->
  Typing_env_types.env ->
  Typing_logic.subtype_prop ->
  unit

val log_tparam_instantiation :
  Typing_env_types.env ->
  Pos.t ->
  'a Typing_defs.tparam ->
  Typing_defs.locl_ty ->
  unit

val log_new_tvar_for_new_object :
  Typing_env_types.env ->
  Pos.t ->
  Typing_defs.locl_ty ->
  'a * string ->
  'b Typing_defs.tparam ->
  unit

val log_new_tvar_for_tconst :
  Typing_env_types.env -> Pos.t -> int -> string -> Typing_defs.locl_ty -> unit

val log_new_tvar_for_tconst_access :
  Typing_env_types.env ->
  Pos.t ->
  Typing_defs.locl_ty ->
  string ->
  string ->
  unit

val log_intersection :
  level:int ->
  Typing_env_types.env ->
  Typing_reason.t ->
  Typing_defs.locl_ty ->
  Typing_defs.locl_ty ->
  inter_ty:Typing_defs.locl_ty ->
  unit

val increment_feature_count : Typing_env_types.env -> string -> unit

// Мост TeaVM -> WIT; память принадлежит одному компоненту и ограничивается хозяином.
#include "plugin.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>
extern int teavm_main(int argc, char **argv);
extern int32_t mcsa_dispatch(int32_t kind, void *data, int32_t len);
static char *output, *event_text;
static bool enabled;
static plugin_list_string_t completions;
static size_t output_len, output_cap;
static void clear(void) { output_len=0; }
static void bytes(const void *p,size_t len) {
    if (output_len+len+1>4*1024*1024+1024) abort();
    if (output_len+len+1>output_cap) { output_cap=(output_len+len+1)*2; output=realloc(output,output_cap); if(!output)abort(); }
    if(len)memcpy(output+output_len,p,len); output_len+=len; output[output_len++]=0;
}
static void text(const char *s) { bytes(s,strlen(s)); }
static void string(plugin_string_t s) { bytes(s.ptr,s.len); }
static void integer(int64_t n) {char s[32];snprintf(s,sizeof(s),"%lld",(long long)n);text(s);}
static void number(double n) {
    // Целые поля моста не требуют дорогого десятичного форматирования double.
    if(n>=INT32_MIN&&n<=INT32_MAX&&n==(int32_t)n&&!(n==0&&__builtin_signbit(n))){integer((int32_t)n);return;}
    char s[64];snprintf(s,sizeof(s),"%.17g",n);text(s);
}
static plugin_string_t str(const char *s) { return (plugin_string_t){(uint8_t*)s,strlen(s)}; }
static void error(plugin_string_t s) { text("!"); string(s); plugin_string_free(&s); }
static void player(mcsa_api_types_player_t *p) {
    string(p->uuid);string(p->name);number(p->game_mode);number(p->dimension);
    number(p->x);number(p->y);number(p->z);number(p->yaw);number(p->pitch);
}
static void hex_bytes(const uint8_t *data,size_t len) {char *s=malloc(len*2+1);if(!s)abort();for(size_t i=0;i<len;i++){s[i*2]="0123456789abcdef"[data[i]>>4];s[i*2+1]="0123456789abcdef"[data[i]&15];}s[len*2]=0;text(s);free(s);}
static int hex_digit(char c) {return c>='0'&&c<='9'?c-'0':c>='a'&&c<='f'?c-'a'+10:-1;}
static bool utf8_text(const uint8_t *data,size_t len) {
    for(size_t i=0;i<len;i++){uint8_t c=data[i];if(!c)return false;if(c<128)continue;
        size_t rest;uint8_t low=128,high=191;
        if(c>=194&&c<=223)rest=1;else if(c>=224&&c<=239){rest=2;if(c==224)low=160;if(c==237)high=159;}else if(c>=240&&c<=244){rest=3;if(c==240)low=144;if(c==244)high=143;}else return false;
        if(i+rest>=len||data[i+1]<low||data[i+1]>high)return false;
        for(size_t j=2;j<=rest;j++)if(data[i+j]<128||data[i+j]>191)return false;i+=rest;
    }return true;
}
int32_t mcsa_length(void) { return (int32_t)output_len; }
static void entity_snapshot(mcsa_api_entities_snapshot_t *p) {
    string(p->uuid);string(p->kind);number(p->at.dimension);number(p->at.x);number(p->at.y);number(p->at.z);number(p->at.yaw.is_some?p->at.yaw.val:0);number(p->at.pitch.is_some?p->at.pitch.val:0);number(p->width);number(p->height);number(p->health);number(p->living);number(p->hostile);number(p->animal);string(p->shooter);number(p->embedded);
}
void *mcsa_call(int32_t op,void *data,int32_t len) {
    char *a[1030]={0}; int count=0; char *copy=malloc((size_t)len+1); if(!copy)abort();
    memcpy(copy,data,len);copy[len]=0;
    for(int i=0;i<len&&count<1030;){a[count++]=copy+i;i+=(int)strlen(copy+i)+1;}
    plugin_string_t s[1030];for(int i=0;i<(count>16?count:16);i++)s[i]=str(a[i]?a[i]:"");
    plugin_string_t err={0};clear();
    switch(op) {
    case 0: mcsa_api_server_broadcast(&s[0]);break;
    case 1: {mcsa_api_server_list_player_t list;mcsa_api_server_ready_players(&list);for(size_t i=0;i<list.len;i++)player(&list.ptr[i]);mcsa_api_server_list_player_free(&list);break;}
    case 2: number(mcsa_api_player_send_message(&s[0],&s[1]));break;
    case 3: {mcsa_api_types_location_t loc={.dimension=atoi(a[1]),.x=atof(a[2]),.y=atof(a[3]),.z=atof(a[4]),.yaw={true,atof(a[5])},.pitch={true,atof(a[6])}};number(mcsa_api_player_teleport(&s[0],&loc));break;}
    case 4: {mcsa_api_command_command_spec_t spec={.name=s[0],.description=s[1],.usage=s[2],.level=atoi(a[3])};if(!mcsa_api_command_register(&spec,&err))error(err);break;}
    case 5: number(mcsa_api_scheduler_run_later(atoi(a[0]),atoi(a[2])));break;
    case 6: number(mcsa_api_scheduler_run_repeating(atoi(a[0]),atoi(a[1]),atoi(a[2])));break;
    case 7: number(mcsa_api_scheduler_cancel(atoi(a[0])));break;
    case 8: {plugin_option_list_u8_t value;if(!mcsa_api_storage_read(&s[0],&value,&err))error(err);else {if(value.is_some){if(!utf8_text(value.val.ptr,value.val.len)){text("!");text("Файл не является текстом UTF-8 без NUL");}else bytes(value.val.ptr,value.val.len);}else text("");plugin_option_list_u8_free(&value);}break;}
    case 26: {plugin_option_list_u8_t value;if(!mcsa_api_storage_read(&s[0],&value,&err))error(err);else {if(!value.is_some){plugin_list_u8_t initial={s[1].ptr,s[1].len};if(!mcsa_api_storage_write(&s[0],&initial,&err))error(err);}plugin_option_list_u8_free(&value);}break;}
    case 9: {plugin_list_u8_t value={s[1].ptr,s[1].len};if(!mcsa_api_storage_write(&s[0],&value,&err))error(err);break;}
    case 10: {uint8_t level=0;mcsa_api_player_operator_level(&s[0],&level);number(level);break;}
    case 11: number(mcsa_api_player_set_game_mode(&s[0],atoi(a[1])));break;
    case 12: {mcsa_api_presentation_player_info_t info;if(mcsa_api_presentation_get_player_info(&s[0],&info)){number(info.health);mcsa_api_presentation_player_info_free(&info);}else {text("!");text("Игрок отсутствует");}break;}
    case 13:case 14: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};if(op==13){plugin_string_t block;if(mcsa_api_world_get_block(&pos,&block)){string(block);plugin_string_free(&block);}else {text("!");text("Чанк не загружен");}}else if(!mcsa_api_world_set_block(&pos,&s[4],&err))error(err);break;}
    case 15: {mcsa_api_types_location_t loc;mcsa_api_world_spawn_location(atoi(a[0]),&loc);number(loc.x);number(loc.y);number(loc.z);break;}
    case 16: number(mcsa_api_world_time(atoi(a[0])));break;
    case 17: mcsa_api_world_set_time(atoi(a[0]),atoi(a[1]));break;
    case 18: {plugin_string_t weather;mcsa_api_world_weather(atoi(a[0]),&weather);string(weather);plugin_string_free(&weather);break;}
    case 19: if(!mcsa_api_world_set_weather(atoi(a[0]),&s[1],12000,&err))error(err);break;
    case 20: number(mcsa_api_player_kick(&s[0],&s[1]));break;
    case 21: {mcsa_api_types_item_stack_t item={s[2],atoi(a[3])};if(!mcsa_api_player_set_inventory_slot(&s[0],atoi(a[1]),item.count?&item:NULL,&err))error(err);break;}
    case 22: free(event_text);event_text=strdup(a[0]);break;
    case 23: mcsa_api_log_write(MCSA_API_LOG_LEVEL_INFO,&s[0]);break;
    case 24: enabled=true;break;
    case 25: if(completions.len<100 && s[0].len<=512) { completions.ptr=realloc(completions.ptr,(completions.len+1)*sizeof(plugin_string_t));if(!completions.ptr)abort();plugin_string_dup(&completions.ptr[completions.len++],a[0]); }break;
    case 27: number(mcsa_api_presentation_send_component(&s[0],&s[1]));break;
    case 28: number(mcsa_api_presentation_set_action_bar(&s[0],&s[1]));break;
    case 29: if(!mcsa_api_player_set_health(&s[0],atof(a[1]),&err))error(err);break;
    case 30: {mcsa_api_player_potion_effect_t effect={.kind=s[1],.duration=atoi(a[2]),.amplifier=atoi(a[3]),.ambient=!strcmp(a[4],"true"),.particles=!strcmp(a[5],"true"),.icon=!strcmp(a[6],"true")};if(!mcsa_api_player_add_potion_effect(&s[0],&effect,&err))error(err);else number(1);break;}
    case 31: if(!mcsa_api_player_remove_potion_effect(&s[0],&s[1],&err))error(err);break;
    case 32: {mcsa_api_world_border_t border;mcsa_api_world_get_border(atoi(a[0]),&border);number(border.size);number(border.center_x);number(border.center_z);break;}
    case 33: {mcsa_api_world_border_t border={atof(a[1]),atof(a[2]),atof(a[3])};if(!mcsa_api_world_set_border(atoi(a[0]),&border,&err))error(err);break;}
    case 34: {plugin_option_list_u8_t value;if(!mcsa_api_storage_read(&s[0],&value,&err))error(err);else {number(value.is_some);plugin_option_list_u8_free(&value);}break;}
    case 35: {bool removed;if(!mcsa_api_storage_delete(&s[0],&removed,&err))error(err);else number(removed);break;}
    case 36: if(!mcsa_api_storage_transfer(&s[0],&s[1],!strcmp(a[2],"true"),!strcmp(a[3],"true"),&err))error(err);break;
    case 37: {mcsa_api_inventory_list_option_item_t items;if(!mcsa_api_inventory_snapshot(&s[0],&items,&err))error(err);else {for(size_t i=0;i<items.len;i++){if(items.ptr[i].is_some){string(items.ptr[i].val.kind);number(items.ptr[i].val.count);string(items.ptr[i].val.metadata);}else {text("air");text("0");text("");}}mcsa_api_inventory_list_option_item_free(&items);}break;}
    case 38: {mcsa_api_inventory_item_t item={.kind=s[2],.count=atoi(a[3]),.metadata=s[4]};if(!mcsa_api_inventory_set(&s[0],atoi(a[1]),item.count?&item:NULL,&err))error(err);break;}
    case 39: {uint32_t selected;if(!mcsa_api_inventory_selected(&s[0],&selected,&err))error(err);else number(selected);break;}
    case 40: {size_t n=s[2].len;if(n>65536||n%2){text("!");text("HTTPS: неверное тело");break;}plugin_list_u8_t body={.ptr=malloc(n/2?n/2:1),.len=n/2};if(!body.ptr)abort();bool valid=true;for(size_t i=0;i<body.len;i++){int hi=hex_digit(a[2][i*2]),lo=hex_digit(a[2][i*2+1]);if(hi<0||lo<0){valid=false;break;}body.ptr[i]=(uint8_t)((hi<<4)|lo);}uint32_t handle;if(!valid){text("!");text("HTTPS: неверное тело");}else if(!mcsa_api_https_start(&s[0],&s[1],&body,&handle,&err))error(err);else number(handle);free(body.ptr);break;}
    case 41: {mcsa_api_https_option_response_t response;if(!mcsa_api_https_poll(strtoul(a[0],NULL,10),&response,&err))error(err);else {if(response.is_some){number(response.val.status);hex_bytes(response.val.body.ptr,response.val.body.len);}mcsa_api_https_option_response_free(&response);}break;}
    case 42: {mcsa_api_attributes_snapshot_t value;if(!mcsa_api_attributes_get(&s[0],&s[1],&value,&err))error(err);else {number(value.base);number(value.value);for(size_t i=0;i<value.modifiers.len;i++){string(value.modifiers.ptr[i].id);number(value.modifiers.ptr[i].amount);number(value.modifiers.ptr[i].operation);}mcsa_api_attributes_snapshot_free(&value);}break;}
    case 122: {mcsa_api_attributes_snapshot_t value;if(!mcsa_api_attributes_get(&s[0],&s[1],&value,&err))error(err);else {for(size_t i=0;i<value.modifiers.len;i++){string(value.modifiers.ptr[i].id);number(value.modifiers.ptr[i].amount);number(value.modifiers.ptr[i].operation);}mcsa_api_attributes_snapshot_free(&value);}break;}
    case 43: if(!mcsa_api_attributes_set_base(&s[0],&s[1],atof(a[2]),&err))error(err);break;
    case 44: {mcsa_api_attributes_modifier_t modifier={.id=s[2],.amount=atof(a[3]),.operation=atoi(a[4])};if(!mcsa_api_attributes_add(&s[0],&s[1],&modifier,&err))error(err);break;}
    case 45: if(!mcsa_api_attributes_remove(&s[0],&s[1],&s[2],&err))error(err);break;
    case 50: {uint32_t handle;if(!mcsa_api_inventory_chest_create(atoi(a[0]),&s[1],&handle,&err))error(err);else number(handle);break;}
    case 51: {mcsa_api_inventory_item_t item={.kind=s[2],.count=atoi(a[3]),.metadata=s[4]};if(!mcsa_api_inventory_chest_set(atoi(a[0]),atoi(a[1]),item.count?&item:NULL,&err))error(err);break;}
    case 52: {mcsa_api_inventory_list_option_item_t items;if(!mcsa_api_inventory_chest_snapshot(atoi(a[0]),&items,&err))error(err);else {for(size_t i=0;i<items.len;i++){if(items.ptr[i].is_some){string(items.ptr[i].val.kind);number(items.ptr[i].val.count);string(items.ptr[i].val.metadata);}else {text("air");text("0");text("");}}mcsa_api_inventory_list_option_item_free(&items);}break;}
    case 53: if(!mcsa_api_inventory_chest_open(&s[0],atoi(a[1]),&err))error(err);break;
    case 54: if(!mcsa_api_inventory_chest_close(&s[0],&err))error(err);break;
    case 55: {uint32_t handle;if(mcsa_api_inventory_chest_current(&s[0],&handle))number(handle);break;}
    case 56: if(!mcsa_api_inventory_chest_release(atoi(a[0]),&err))error(err);break;
    case 57: {mcsa_api_inventory_option_item_t item;if(!mcsa_api_inventory_cursor(&s[0],&item,&err))error(err);else {if(item.is_some){string(item.val.kind);number(item.val.count);string(item.val.metadata);}mcsa_api_inventory_option_item_free(&item);}break;}
    case 58: {mcsa_api_presentation_team_t team={.id=s[1],.prefix=s[2],.suffix=s[3],.color=s[4],.visibility=s[5],.members={.ptr=s+6,.len=count-6}};number(mcsa_api_presentation_set_team(&s[0],&team));break;}
    case 59: number(mcsa_api_presentation_remove_team(&s[0],&s[1]));break;
    case 60: number(mcsa_api_server_current_tick());break;
    case 61: {mcsa_api_inventory_view_t view;if(!mcsa_api_inventory_open_view(&s[0],&view,&err))error(err);else {string(view.kind);for(size_t i=0;i<view.items.len;i++){if(view.items.ptr[i].is_some){string(view.items.ptr[i].val.kind);number(view.items.ptr[i].val.count);string(view.items.ptr[i].val.metadata);}else {text("air");text("0");text("");}}mcsa_api_inventory_view_free(&view);}break;}
    case 62: {mcsa_api_presentation_player_info_t info;if(mcsa_api_presentation_get_player_info(&s[0],&info)){number(info.food);number(info.level);number(info.exp);mcsa_api_presentation_player_info_free(&info);}break;}
    case 63: {mcsa_api_types_block_pos_t pos={atoi(a[1]),atoi(a[2]),atoi(a[3]),atoi(a[4])};mcsa_api_player_bed_action_t action;if(mcsa_api_player_get_bed_action(&s[0],&pos,&action)){number(action.bed.dimension);number(action.bed.x);number(action.bed.y);number(action.bed.z);number(action.problem);number(action.can_spawn);number(action.can_sleep);}break;}
    case 64: {mcsa_api_types_location_t to={.dimension=atoi(a[1]),.x=atof(a[2]),.y=atof(a[3]),.z=atof(a[4]),.yaw={true,atof(a[5])},.pitch={true,0}};if(!mcsa_api_player_set_respawn(&s[0],!strcmp(a[6],"1")?&to:NULL,&err))error(err);break;}
    case 65: {mcsa_api_player_state_t state;if(!mcsa_api_player_get_state(&s[0],&state,&err))error(err);else {number(state.fire_ticks);number(state.dead);number(state.sneaking);number(state.on_ground);number(state.in_water);number(state.in_lava);number(state.in_rain);number(state.eye_height);number(state.using_item);number(state.off_hand);for(size_t i=0;i<state.effects.len;i++){mcsa_api_player_potion_effect_t e=state.effects.ptr[i];string(e.kind);number(e.duration);number(e.amplifier);number(e.ambient);number(e.particles);number(e.icon);}mcsa_api_player_state_free(&state);}break;}
    case 66: if(!mcsa_api_player_set_fire_ticks(&s[0],atoi(a[1]),&err))error(err);break;
    case 67: if(!mcsa_api_player_damage(&s[0],atof(a[1]),&err))error(err);break;
    case 68: if(!mcsa_api_player_give_experience(&s[0],atoi(a[1]),&err))error(err);break;
    case 69: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};mcsa_api_world_block_state_t state;if(!mcsa_api_world_block_details(&pos,&state,&err))error(err);else {string(state.data);number(state.sky);number(state.light);number(state.liquid);number(state.passable);number(state.age);number(state.max_age);mcsa_api_world_block_state_free(&state);}break;}
    case 70: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};bool result;if(!mcsa_api_world_update_block(&pos,&s[4],!strcmp(a[5],"true"),!strcmp(a[6],"true"),&result,&err))error(err);else number(result);break;}
    case 71: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};if(!mcsa_api_world_random_tick(&pos,&err))error(err);break;}
    case 72: number(mcsa_api_world_chunk_loaded(atoi(a[0]),atoi(a[1]),atoi(a[2])));break;
    case 73: {int32_t y;if(!mcsa_api_world_highest_block(atoi(a[0]),atoi(a[1]),atoi(a[2]),&y,&err))error(err);else number(y);break;}
    case 74: number(mcsa_api_world_random_tick_speed());break;
    case 75: {mcsa_api_types_location_t at={.dimension=atoi(a[0]),.x=atof(a[1]),.y=atof(a[2]),.z=atof(a[3])};if(!mcsa_api_world_play_sound(&at,&s[4],atof(a[5]),atof(a[6]),&err))error(err);break;}
    case 76: {mcsa_api_entities_vector_t v;if(!mcsa_api_entities_velocity(&s[0],&v,&err))error(err);else {number(v.x);number(v.y);number(v.z);}break;}
    case 77: {mcsa_api_entities_vector_t v={atof(a[1]),atof(a[2]),atof(a[3])};if(!mcsa_api_entities_set_velocity(&s[0],&v,&err))error(err);break;}
    case 78: if(!mcsa_api_entities_remove(&s[0],&err))error(err);break;
    case 79: {mcsa_api_types_location_t at={.dimension=atoi(a[0]),.x=atof(a[1]),.y=atof(a[2]),.z=atof(a[3])};mcsa_api_inventory_item_t item={.kind=s[4],.count=atoi(a[5]),.metadata=s[6]};plugin_string_t uuid;if(!mcsa_api_entities_drop_item(&at,&item,!strcmp(a[7],"true"),&uuid,&err))error(err);else {string(uuid);plugin_string_free(&uuid);}break;}
    case 80: {mcsa_api_inventory_item_t item;if(!mcsa_api_entities_item_stack(&s[0],&item,&err))error(err);else {string(item.kind);number(item.count);string(item.metadata);mcsa_api_inventory_item_free(&item);}break;}
    case 81: {mcsa_api_inventory_item_t item={.kind=s[1],.count=atoi(a[2]),.metadata=s[3]};if(!mcsa_api_entities_set_item_stack(&s[0],&item,&err))error(err);break;}
    case 82: {mcsa_api_types_location_t at;if(!mcsa_api_entities_item_location(&s[0],&at,&err))error(err);else {number(at.dimension);number(at.x);number(at.y);number(at.z);}break;}
    case 83: {mcsa_api_types_location_t at={.dimension=atoi(a[1]),.x=atof(a[2]),.y=atof(a[3]),.z=atof(a[4])};bool result;if(!mcsa_api_entities_item_teleport(&s[0],&at,&result,&err))error(err);else number(result);break;}
    case 84: number(mcsa_api_entities_valid(&s[0]));break;
    case 86: {mcsa_api_types_location_t at={.dimension=atoi(a[0]),.x=atof(a[1]),.y=atof(a[2]),.z=atof(a[3])};if(!mcsa_api_world_spawn_particle(&at,&s[4],atoi(a[5]),atof(a[6]),atof(a[7]),atof(a[8]),&err))error(err);break;}
    case 87: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};bool result;if(!mcsa_api_world_break_naturally(&pos,&result,&err))error(err);else number(result);break;}
    case 88: number(mcsa_api_server_is_stopping());break;
    case 89: {mcsa_api_player_flight_t state;if(!mcsa_api_player_get_flight(&s[0],&state,&err))error(err);else {number(state.allowed);number(state.flying);number(state.fall_damage);}break;}
    case 90: if(!mcsa_api_player_set_flight(&s[0],atoi(a[1]),atoi(a[2]),&err))error(err);break;
    case 91: {mcsa_api_types_location_t at={.dimension=atoi(a[0]),.x=atof(a[1]),.y=atof(a[2]),.z=atof(a[3])};mcsa_api_world_option_ray_hit_t hit;if(!mcsa_api_world_ray_blocks(&at,atof(a[4]),atof(a[5]),atof(a[6]),atof(a[7]),atoi(a[8]),!strcmp(a[9],"true"),&hit,&err))error(err);else if(hit.is_some){number(hit.val.x);number(hit.val.y);number(hit.val.z);number(hit.val.block.x);number(hit.val.block.y);number(hit.val.block.z);number(hit.val.face);}break;}
    case 92: number(mcsa_api_world_monster_limit(atoi(a[0])));break;
    case 93: if(!mcsa_api_world_set_monster_limit(atoi(a[0]),atoi(a[1]),&err))error(err);break;
    case 94: {mcsa_api_entities_snapshot_t state;if(!mcsa_api_entities_describe(&s[0],&state,&err))error(err);else {entity_snapshot(&state);mcsa_api_entities_snapshot_free(&state);}break;}
    case 95: {mcsa_api_entities_list_snapshot_t states;if(!mcsa_api_entities_in_world(atoi(a[0]),&states,&err))error(err);else {for(size_t i=0;i<states.len;i++)entity_snapshot(&states.ptr[i]);mcsa_api_entities_list_snapshot_free(&states);}break;}
    case 96: {mcsa_api_entities_list_potion_effect_t effects;if(!mcsa_api_entities_effects(&s[0],&effects,&err))error(err);else {for(size_t i=0;i<effects.len;i++){mcsa_api_player_potion_effect_t *effect=&effects.ptr[i];string(effect->kind);number(effect->duration);number(effect->amplifier);number(effect->ambient);number(effect->particles);number(effect->icon);}mcsa_api_entities_list_potion_effect_free(&effects);}break;}
    case 97: {mcsa_api_entities_option_item_t stack;if(!mcsa_api_entities_equipment(&s[0],atoi(a[1]),&stack,&err))error(err);else {if(stack.is_some){string(stack.val.kind);number(stack.val.count);string(stack.val.metadata);}mcsa_api_entities_option_item_free(&stack);}break;}
    case 98: if(!mcsa_api_entities_set_health(&s[0],atof(a[1]),&err))error(err);break;
    case 99: {mcsa_api_types_location_t at={.dimension=atoi(a[1]),.x=atof(a[2]),.y=atof(a[3]),.z=atof(a[4]),.yaw={true,atof(a[5])},.pitch={true,atof(a[6])}};bool result;if(!mcsa_api_entities_teleport(&s[0],&at,&result,&err))error(err);else number(result);break;}
    case 100: if(!mcsa_api_entities_set_arrow_critical(&s[0],!strcmp(a[1],"true"),&err))error(err);break;
    case 101: {mcsa_api_inventory_item_t stack={.kind=s[1],.count=atoi(a[2]),.metadata=s[3]};if(!mcsa_api_entities_set_potion_item(&s[0],&stack,&err))error(err);break;}
    case 102: {mcsa_api_entities_vector_t velocity={atof(a[2]),atof(a[3]),atof(a[4])};plugin_string_t uuid;if(!mcsa_api_entities_launch(&s[0],&s[1],&velocity,&uuid,&err))error(err);else {string(uuid);plugin_string_free(&uuid);}break;}
    case 103: {float power=atof(a[2]);bool fire=atoi(a[2]);if(!mcsa_api_entities_set_explosive(&s[0],!strcmp(a[1],"0")?&power:NULL,!strcmp(a[1],"1")?&fire:NULL,&err))error(err);break;}
    case 104: {mcsa_api_entities_horse_state_t state;if(!mcsa_api_entities_horse(&s[0],&state,&err))error(err);else {number(state.tamed);number(state.chest);for(size_t i=0;i<state.items.len;i++){if(state.items.ptr[i].is_some){string(state.items.ptr[i].val.kind);number(state.items.ptr[i].val.count);string(state.items.ptr[i].val.metadata);}else {text("air");text("0");text("");}}mcsa_api_entities_horse_state_free(&state);}break;}
    case 105: {mcsa_api_inventory_item_t item={.kind=s[2],.count=atoi(a[3]),.metadata=s[4]};if(!mcsa_api_entities_set_horse_item(&s[0],atoi(a[1]),item.count?&item:NULL,&err))error(err);break;}
    case 106: if(!strcmp(a[1],"0")){if(!mcsa_api_entities_tame_horse(&s[0],atoi(a[2]),&err))error(err);}else {if(!mcsa_api_entities_set_horse_owner(&s[0],s[2].len?&s[2]:NULL,&err))error(err);}break;
    case 107: {mcsa_api_types_location_t at={.dimension=atoi(a[0]),.x=atof(a[1]),.y=atof(a[2]),.z=atof(a[3]),.yaw={true,atof(a[4])},.pitch={true,atof(a[5])}};plugin_string_t uuid;if(!mcsa_api_entities_create_mob(&at,&s[6],&uuid,&err))error(err);else {string(uuid);plugin_string_free(&uuid);}break;}
    case 108: if(!mcsa_api_entities_finish_mob(&s[0],atoi(a[1]),&err))error(err);break;
    case 110: if(!mcsa_api_entities_mob_control(&s[0],atoi(a[1]),atoi(a[2]),&err))error(err);break;
    case 111: {plugin_option_string_t target;if(!mcsa_api_entities_get_target(&s[0],&target,&err))error(err);else {if(target.is_some)string(target.val);plugin_option_string_free(&target);}break;}
    case 112: if(!mcsa_api_entities_set_target(&s[0],s[1].len?&s[1]:NULL,&err))error(err);break;
    case 113: {mcsa_api_inventory_item_t stack={.kind=s[0],.count=atoi(a[1]),.metadata=s[2]},result;if(!mcsa_api_inventory_enchant_with_levels(&stack,atoi(a[3]),!strcmp(a[4],"true"),strtoll(a[5],NULL,10),&result,&err))error(err);else {string(result.kind);number(result.count);string(result.metadata);mcsa_api_inventory_item_free(&result);}break;}
    case 117: if(!mcsa_api_player_set_silent(&s[0],atoi(a[1]),&err))error(err);break;
    case 118: {bool silent;if(!mcsa_api_player_get_silent(&s[0],&silent,&err))error(err);else number(silent);break;}
    case 116: {plugin_option_string_t uuid;if(!mcsa_api_player_vehicle(&s[0],&uuid,&err))error(err);else {if(uuid.is_some)string(uuid.val);plugin_option_string_free(&uuid);}break;}
    case 115: {bool inside;if(!mcsa_api_player_inside_vehicle(&s[0],&inside,&err))error(err);else number(inside);break;}
    case 114: {plugin_option_string_t type;if(!mcsa_api_entities_cloud_potion(&s[0],&type,&err))error(err);else {if(type.is_some)string(type.val);plugin_option_string_free(&type);}break;}
    case 161: {mcsa_api_types_block_pos_t pos={atoi(a[0]),atoi(a[1]),atoi(a[2]),atoi(a[3])};plugin_list_string_t blocks;if(!mcsa_api_world_block_cube(&pos,&blocks,&err))error(err);else {// Одинаковые блоки передаём сериями: без сотен копий строк air в Java-куче.
        for(size_t i=0;i<blocks.len;){size_t end=i+1;while(end<blocks.len&&blocks.ptr[end].len==blocks.ptr[i].len&&!memcmp(blocks.ptr[end].ptr,blocks.ptr[i].ptr,blocks.ptr[i].len))end++;number(end-i);string(blocks.ptr[i]);i=end;}plugin_list_string_free(&blocks);}break;}
    default: abort();
    }
    free(copy);return output;
}
static int dispatch(int kind) {
    size_t len=output_len;char *input=malloc(len?len:1);if(!input)abort();if(len)memcpy(input,output,len);
    free(event_text);event_text=NULL;int result=mcsa_dispatch(kind,input,(int32_t)len);free(input);return result;
}
void exports_mcsa_api_lifecycle_enable(void) { char *args[]={"plugin",NULL};teavm_main(1,args);if(!enabled)abort(); }
void exports_mcsa_api_lifecycle_disable(void) { clear();dispatch(0);free(event_text);event_text=NULL;free(output);output=NULL;output_len=output_cap=0; }
void exports_mcsa_api_events_on_player_join(mcsa_api_types_player_t *who) {clear();player(who);dispatch(1);}
void exports_mcsa_api_events_on_player_quit(mcsa_api_types_player_t *who) {clear();player(who);dispatch(2);}
void exports_mcsa_api_events_on_chat(mcsa_api_types_player_t *who,plugin_string_t *message,mcsa_api_types_chat_result_t *ret) {
    clear();player(who);string(*message);int result=dispatch(3);ret->tag=result;
    if(result==2)plugin_string_dup(&ret->val.replace,event_text?event_text:"");
}
static uint8_t block_event(int kind,mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *block) {
    clear();player(who);number(pos->dimension);number(pos->x);number(pos->y);number(pos->z);string(*block);return dispatch(kind)?1:0;
}
uint8_t exports_mcsa_api_events_on_block_break(mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *block) {return block_event(4,who,pos,block);}
uint8_t exports_mcsa_api_events_on_block_place(mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *block) {return block_event(5,who,pos,block);}
uint8_t exports_mcsa_api_events_on_player_command(mcsa_api_types_player_t *who,plugin_string_t *line) {clear();player(who);string(*line);return dispatch(6)?1:0;}
void exports_mcsa_api_events_on_command(mcsa_api_types_sender_t *caller,plugin_string_t *name,plugin_list_string_t *args,mcsa_api_types_command_result_t *ret) {
    clear();number(caller->kind);if(caller->uuid.is_some)string(caller->uuid.val);else text("");string(caller->name);string(*name);
    for(size_t i=0;i<args->len;i++)string(args->ptr[i]);ret->success=dispatch(7)!=0;ret->message.is_some=false;
}
void exports_mcsa_api_events_on_task(uint32_t task) {
 clear();number(task);dispatch(8);
}

bool exports_mcsa_api_combat_events_on_damage(exports_mcsa_api_combat_events_player_t *who,plugin_string_t *cause,double damage,double *ret) {
    clear();player(who);string(*cause);number(damage);int result=dispatch(10);
    if(result==1)return false;
    *ret=result==2&&event_text?atof(event_text):damage;return true;
}
bool exports_mcsa_api_combat_events_on_damage_from(exports_mcsa_api_combat_events_player_t *who,exports_mcsa_api_combat_events_player_t *attacker,plugin_string_t *cause,double damage,double *ret) {
    clear();player(who);string(*cause);number(damage);number(attacker!=NULL);if(attacker)player(attacker);int result=dispatch(18);
    if(result==1)return false;*ret=result==2&&event_text?atof(event_text):damage;return true;
}
void exports_mcsa_api_combat_events_on_death(exports_mcsa_api_combat_events_player_t *who,bool keep_inventory) {
    clear();player(who);number(keep_inventory);dispatch(11);
}

void exports_mcsa_api_completion_on_tab_complete(mcsa_api_types_sender_t *caller,plugin_string_t *name,plugin_list_string_t *args,plugin_list_string_t *ret) {
    clear();number(caller->kind);if(caller->uuid.is_some)string(caller->uuid.val);else text("");string(caller->name);string(*name);
    for(size_t i=0;i<args->len;i++)string(args->ptr[i]);
    completions=(plugin_list_string_t){0};dispatch(9);*ret=completions;completions=(plugin_list_string_t){0};
}

bool exports_mcsa_api_inventory_events_on_inventory(mcsa_api_types_player_t *who,uint32_t handle,uint32_t kind,int32_t slot,int32_t button,int32_t mode) {
    clear();player(who);number(handle);number(kind);number(slot);number(button);number(mode);return dispatch(12)!=0;
}

bool exports_mcsa_api_item_events_on_item(mcsa_api_types_player_t *who,uint32_t kind,int32_t slot) {
    clear();player(who);number(kind);number(slot);return dispatch(16)!=0;
}

bool exports_mcsa_api_item_events_on_drag(mcsa_api_types_player_t *who,uint32_t handle,plugin_list_s32_t *slots) {
    clear();player(who);number(handle);for(size_t i=0;i<slots->len;i++)number(slots->ptr[i]);return dispatch(17)!=0;
}

bool exports_mcsa_api_player_events_on_move(mcsa_api_types_player_t *who,mcsa_api_types_location_t *destination,bool jump,mcsa_api_types_location_t *ret) {
    clear();player(who);number(destination->dimension);number(destination->x);number(destination->y);number(destination->z);number(destination->yaw.is_some?destination->yaw.val:who->yaw);number(destination->pitch.is_some?destination->pitch.val:who->pitch);number(jump);
    int result=dispatch(13);if(result==1)return false;*ret=*destination;
    if(result==2&&event_text){unsigned dimension;double x,y,z;float yaw,pitch;if(sscanf(event_text,"%u %lf %lf %lf %f %f",&dimension,&x,&y,&z,&yaw,&pitch)!=6||dimension>2)abort();ret->dimension=dimension;ret->x=x;ret->y=y;ret->z=z;ret->yaw.is_some=ret->pitch.is_some=true;ret->yaw.val=yaw;ret->pitch.val=pitch;}return true;
}
uint32_t exports_mcsa_api_player_events_on_interact(mcsa_api_types_player_t *who,uint32_t action,uint32_t hand,mcsa_api_types_block_pos_t *block,uint32_t face) {
    clear();player(who);number(action);number(hand);number(block!=NULL);number(block?block->dimension:who->dimension);number(block?block->x:0);number(block?block->y:0);number(block?block->z:0);number(face);return (uint32_t)dispatch(14);
}
void exports_mcsa_api_player_events_on_respawn(mcsa_api_types_player_t *who,mcsa_api_types_location_t *destination,bool bed,bool anchor,mcsa_api_types_location_t *ret) {
    clear();player(who);number(destination->dimension);number(destination->x);number(destination->y);number(destination->z);number(destination->yaw.is_some?destination->yaw.val:who->yaw);number(destination->pitch.is_some?destination->pitch.val:who->pitch);number(bed);number(anchor);
    dispatch(15);*ret=*destination;unsigned dimension;double x,y,z;float yaw,pitch;if(!event_text||sscanf(event_text,"%u %lf %lf %lf %f %f",&dimension,&x,&y,&z,&yaw,&pitch)!=6||dimension>2)abort();ret->dimension=dimension;ret->x=x;ret->y=y;ret->z=z;ret->yaw.is_some=ret->pitch.is_some=true;ret->yaw.val=yaw;ret->pitch.val=pitch;
}

bool exports_mcsa_api_item_events_on_entity(mcsa_api_types_player_t *who,plugin_string_t *target,uint32_t hand,uint32_t *slot) {clear();player(who);string(*target);number(hand);number(slot?(int64_t)*slot:-1);return dispatch(19)==1;}

bool exports_mcsa_api_item_events_on_prepare_craft(mcsa_api_types_player_t *who,exports_mcsa_api_item_events_list_option_item_t *grid,mcsa_api_inventory_item_t *result,bool repair,bool table,mcsa_api_inventory_item_t *ret) {
    clear();player(who);number(repair);number(table);if(result){string(result->kind);number(result->count);string(result->metadata);}else {text("air");text("0");text("");}
    for(size_t i=0;i<grid->len;i++){if(grid->ptr[i].is_some){string(grid->ptr[i].val.kind);number(grid->ptr[i].val.count);string(grid->ptr[i].val.metadata);}else {text("air");text("0");text("");}}
    dispatch(20);if(!event_text||!event_text[0])return false;char *count=strchr(event_text,'\n');if(!count)abort();*count++=0;char *metadata=strchr(count,'\n');if(!metadata)abort();*metadata++=0;plugin_string_dup(&ret->kind,event_text);ret->count=atoi(count);plugin_string_dup(&ret->metadata,metadata);return true;
}

int32_t exports_mcsa_api_item_events_on_prepare_anvil(mcsa_api_types_player_t *who,exports_mcsa_api_item_events_list_option_item_t *inputs,mcsa_api_inventory_item_t *result,int32_t cost) {
    clear();player(who);number(cost);for(size_t i=0;i<inputs->len;i++){if(inputs->ptr[i].is_some){string(inputs->ptr[i].val.kind);number(inputs->ptr[i].val.count);string(inputs->ptr[i].val.metadata);}else {text("air");text("0");text("");}}if(result){string(result->kind);number(result->count);string(result->metadata);}else {text("air");text("0");text("");}dispatch(21);return event_text?atoi(event_text):cost;
}

void exports_mcsa_api_item_events_on_prepare_enchant(mcsa_api_types_player_t *who,mcsa_api_inventory_item_t *item,plugin_list_s32_t *properties,plugin_list_s32_t *ret) {
    clear();player(who);string(item->kind);number(item->count);string(item->metadata);for(size_t i=0;i<properties->len;i++)number(properties->ptr[i]);dispatch(22);ret->len=0;ret->ptr=NULL;if(!event_text||!*event_text)return;ret->ptr=malloc(10*sizeof(int32_t));if(!ret->ptr)abort();char *p=event_text;for(int i=0;i<10;i++){char *end;ret->ptr[i]=strtol(p,&end,10);if(end==p)abort();p=end;}ret->len=10;
}
bool exports_mcsa_api_item_events_on_enchant(mcsa_api_types_player_t *who,mcsa_api_inventory_item_t *item,mcsa_api_inventory_item_t *result,uint32_t button,int32_t cost,plugin_string_t *hint,int32_t level,mcsa_api_inventory_item_t *ret) {
    clear();player(who);string(item->kind);number(item->count);string(item->metadata);string(result->kind);number(result->count);string(result->metadata);number(button);number(cost);string(*hint);number(level);dispatch(23);if(!event_text||!*event_text)return false;char *count=strchr(event_text,'\n');if(!count)abort();*count++=0;char *metadata=strchr(count,'\n');if(!metadata)abort();*metadata++=0;plugin_string_dup(&ret->kind,event_text);ret->count=atoi(count);plugin_string_dup(&ret->metadata,metadata);return true;
}

bool exports_mcsa_api_item_events_on_food(mcsa_api_types_player_t *who,int32_t level,mcsa_api_inventory_item_t *item,int32_t *ret) {clear();player(who);number(level);if(item){string(item->kind);number(item->count);string(item->metadata);}else {text("air");text("0");text("");}if(dispatch(24)==1)return false;*ret=event_text?atoi(event_text):level;return true;}

void exports_mcsa_api_trading_events_on_trade(mcsa_api_types_player_t *who,bool closed,plugin_list_string_t *slots,plugin_list_string_t *offers,plugin_list_s32_t *ret) {
 clear();player(who);number(closed);for(size_t i=0;i<slots->len;i++)string(slots->ptr[i]);for(size_t i=0;i<offers->len;i++)string(offers->ptr[i]);dispatch(25);
 ret->len=0;ret->ptr=NULL;if(!event_text)return;size_t count=offers->len/10+1;ret->ptr=malloc(count*sizeof(int32_t));if(!ret->ptr)abort();char *p=event_text;for(size_t i=0;i<count;i++){char *end;long value=strtol(p,&end,10);if(end==p||value<INT32_MIN||value>INT32_MAX)abort();ret->ptr[i]=value;p=end;}ret->len=count;
}

int32_t exports_mcsa_api_item_events_on_furnace(mcsa_api_types_block_pos_t *pos,int32_t total) {clear();number(pos->dimension);number(pos->x);number(pos->y);number(pos->z);number(total);dispatch(26);return event_text?atoi(event_text):total;}

bool exports_mcsa_api_generation_events_wants_population(mcsa_api_types_dimension_t dimension) {clear();number(dimension);return dispatch(27)==1;}
void exports_mcsa_api_generation_events_on_world_init(mcsa_api_types_dimension_t dimension) {clear();number(dimension);dispatch(29);}
void exports_mcsa_api_generation_events_on_populate(mcsa_api_types_dimension_t dimension,int32_t x,int32_t z,int64_t seed,plugin_list_string_t *palette,plugin_list_u32_t *runs,plugin_list_string_t *ret) {
 clear();number(dimension);number(x);number(z);integer(seed);number(palette->len);for(size_t i=0;i<palette->len;i++)string(palette->ptr[i]);// Числа серий идут одним полем: без десятков тысяч временных Java-строк.
 char *packed=malloc(runs->len*12+1);if(!packed)abort();size_t used=0;
 for(size_t i=0;i<runs->len;i++){uint32_t value=runs->ptr[i];char digits[10];size_t n=0;do{digits[n++]=(char)('0'+value%10);value/=10;}while(value);while(n)packed[used++]=digits[--n];packed[used++]=',';}
 bytes(packed,used);free(packed);
 // Канонический ABI передаёт входные списки во владение гостю.
 // После сериализации они больше не нужны; иначе каждый чанк оставляет утечку.
 plugin_list_string_free(palette);plugin_list_u32_free(runs);dispatch(28);ret->ptr=NULL;ret->len=0;if(!event_text||!*event_text)return;
 size_t count=0;for(char *p=event_text;*p;p++)if(*p=='\n')count++;if(count>98304)abort();ret->ptr=calloc(count,sizeof(plugin_string_t));if(!ret->ptr)abort();char *start=event_text;for(char *p=start;*p;p++)if(*p=='\n'){*p=0;plugin_string_dup(&ret->ptr[ret->len++],start);start=p+1;}
}

bool exports_mcsa_api_item_events_on_ground_item(mcsa_api_types_player_t *who,plugin_string_t *item,bool pickup,int32_t remaining) {clear();player(who);string(*item);number(pickup);number(remaining);return dispatch(30)==1;}

void exports_mcsa_api_item_events_on_block_drop(mcsa_api_types_player_t *who,mcsa_api_types_block_pos_t *pos,plugin_string_t *data,plugin_list_string_t *items,plugin_list_string_t *ret) {
 clear();player(who);number(pos->dimension);number(pos->x);number(pos->y);number(pos->z);string(*data);for(size_t i=0;i<items->len;i++)string(items->ptr[i]);dispatch(31);ret->ptr=NULL;ret->len=0;if(!event_text||!*event_text)return;ret->ptr=calloc(items->len,sizeof(plugin_string_t));if(!ret->ptr)abort();char *start=event_text;for(char *p=start;*p;p++)if(*p=='\n'){if(ret->len>=items->len)abort();*p=0;plugin_string_dup(&ret->ptr[ret->len++],start);start=p+1;}
}

void exports_mcsa_api_native_events_on_event(uint32_t kind,plugin_list_string_t *fields,plugin_list_string_t *ret) {
 clear();number(kind);for(size_t i=0;i<fields->len;i++)string(fields->ptr[i]);dispatch(32);ret->ptr=NULL;ret->len=0;if(!event_text||!*event_text)return;
 size_t count=0;for(char *p=event_text;*p;p++)if(*p=='\n')count++;if(count>100000)abort();ret->ptr=calloc(count,sizeof(plugin_string_t));if(!ret->ptr)abort();char *start=event_text;for(char *p=start;*p;p++)if(*p=='\n'){*p=0;plugin_string_dup(&ret->ptr[ret->len++],start);start=p+1;}
}

void exports_mcsa_api_item_events_on_entity_death(mcsa_api_types_player_t *who,plugin_string_t *entity,bool keep,exports_mcsa_api_item_events_list_item_t *drops,exports_mcsa_api_item_events_list_item_t *ret) {
 clear();number(who!=NULL);if(who)player(who);string(*entity);number(keep);for(size_t i=0;i<drops->len;i++){string(drops->ptr[i].kind);number(drops->ptr[i].count);string(drops->ptr[i].metadata);}dispatch(33);ret->ptr=NULL;ret->len=0;if(!event_text||!*event_text)return;
 size_t count=0;for(char *p=event_text;*p;p++)if(*p=='\n')count++;if(count%3||count>1536)abort();ret->ptr=calloc(count/3,sizeof(mcsa_api_inventory_item_t));if(!ret->ptr)abort();char *start=event_text;size_t field=0;for(char *p=start;*p;p++)if(*p=='\n'){*p=0;mcsa_api_inventory_item_t *item=&ret->ptr[field/3];if(field%3==0)plugin_string_dup(&item->kind,start);else if(field%3==1)item->count=strtoul(start,NULL,10);else {plugin_string_dup(&item->metadata,start);ret->len++;}field++;start=p+1;}
}

void exports_mcsa_api_generation_events_on_loot(mcsa_api_types_block_pos_t *pos,exports_mcsa_api_generation_events_list_option_item_t *slots,exports_mcsa_api_generation_events_list_option_item_t *ret) {
 clear();number(pos->dimension);number(pos->x);number(pos->y);number(pos->z);for(size_t i=0;i<slots->len;i++){if(slots->ptr[i].is_some){string(slots->ptr[i].val.kind);number(slots->ptr[i].val.count);string(slots->ptr[i].val.metadata);}else{text("air");text("0");text("");}}dispatch(34);ret->len=slots->len;ret->ptr=calloc(ret->len,sizeof(*ret->ptr));if(!ret->ptr)abort();if(!event_text)abort();char *p=event_text;for(size_t i=0;i<ret->len;i++){char *type=p,*count=strchr(p,'\n');if(!count)abort();*count++=0;char *metadata=strchr(count,'\n');if(!metadata)abort();*metadata++=0;char *end=strchr(metadata,'\n');if(!end)abort();*end=0;p=end+1;ret->ptr[i].is_some=strcmp(type,"air")&&atoi(count)>0;if(ret->ptr[i].is_some){plugin_string_dup(&ret->ptr[i].val.kind,type);ret->ptr[i].val.count=strtoul(count,NULL,10);plugin_string_dup(&ret->ptr[i].val.metadata,metadata);}}
}

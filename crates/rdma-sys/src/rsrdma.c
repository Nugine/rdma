#include "bindings.h"

int rs_ibv_query_device_ex(                        //
    struct ibv_context *context,                   //
    const struct ibv_query_device_ex_input *input, //
    struct ibv_device_attr_ex *attr                //
) {
    return ibv_query_device_ex(context, input, attr);
}

int rs_ibv_query_port(              //
    struct ibv_context *context,    //
    uint8_t port_num,               //
    struct ibv_port_attr *port_attr //
) {
    return ibv_query_port(context, port_num, port_attr);
}

int rs_ibv_query_gid_ex(         //
    struct ibv_context *context, //
    uint32_t port_num,           //
    uint32_t gid_index,          //
    struct ibv_gid_entry *entry, //
    uint32_t flags               //
) {
    return ibv_query_gid_ex(context, port_num, gid_index, entry, flags);
}

struct ibv_cq_ex *rs_ibv_create_cq_ex(  //
    struct ibv_context *context,        //
    struct ibv_cq_init_attr_ex *cq_attr //
) {
    return ibv_create_cq_ex(context, cq_attr);
}
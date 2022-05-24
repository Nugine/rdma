#include <infiniband/verbs.h>

int rs_ibv_query_device_ex(                        //
    struct ibv_context *context,                   //
    const struct ibv_query_device_ex_input *input, //
    struct ibv_device_attr_ex *attr                //
) {
    return ibv_query_device_ex(context, input, attr);
}

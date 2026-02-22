#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum StreamType {
  Audio,
  Video,
} StreamType;

typedef void (*ReleaseCallback)(void*);

bool rust_send_frame(const uint8_t *data,
                     uintptr_t len,
                     void *context,
                     ReleaseCallback release_callback,
                     uint32_t timestamp);

void run_runtime_server(enum StreamType stream);

extern void swift_receive_frame(void *context, void *frameData, uintptr_t frameDataLength);

extern void *swift_receive_pps_sps(void *context,
                                   const uint8_t *pps,
                                   uintptr_t pps_length,
                                   const uint8_t *sps,
                                   uintptr_t sps_length,
                                   const uint8_t *addr);

void rust_set_signalling_addr(const uint8_t *host_addr, uintptr_t host_addr_length);

void rust_send_video_callback(void *context);

void rust_send_h264_config(const uint8_t *pps,
                           uintptr_t pps_length,
                           const uint8_t *sps,
                           uintptr_t sps_length);

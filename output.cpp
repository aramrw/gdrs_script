#include <stdio.h>
#include <stdint.h>

int user_main() {
    const int32_t x = 10LL;
    int32_t y = 20LL;
    const int32_t z = x;
    printf("%lld\n", x);
    printf("%lld\n", y);
    printf("%lld\n", z);
    return 0;
}

int main() {
    return user_main();
}

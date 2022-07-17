# actix-web 更加灵活的身份认证拦截实现

> 本文所使用的 actix-web 版本为 4.1.0

通常实现身份认证拦截的时候，我们会想到用中间件。比如 [actix-web-httpauth](https://github.com/actix/actix-extras/tree/master/actix-web-httpauth) ，但是在使用的时候却不够灵活。比如某个接口不需要拦截，或者在登陆和未登陆的时候返回两种不同的响应。

本文带大家从 actix-web 的 [Handler\<Args>](https://docs.rs/actix-web/latest/actix_web/trait.Handler.html) 和 [FromRequest](https://docs.rs/actix-web/latest/actix_web/trait.FromRequest.html) 认识一个好玩的方法。

## 原理基础

> 提示：如果你已经知道了原理，或者不期望知道原理，可以跳过本节

在 actix-web 中，[路由设置 handler 的方法](https://docs.rs/actix-web/latest/actix_web/struct.Route.html#method.to) 定义如下：

```rust
pub fn to<F, Args>(self, handler: F) -> Self
where
    F: Handler<Args>,
    Args: FromRequest + 'static,
    F::Output: Responder + 'static,
{
    self.service = handler_service(handler);
    self
}
```

只调用了 `handler_service` 方法， 参数 handler 的约束为 `Handler<Args>`, 其定义如下：

```rust
pub trait Handler<Args>: Clone + 'static {
    type Output;
    type Future: Future<Output = Self::Output>;

    fn call(&self, args: Args) -> Self::Future;
}
```

在这里我们能够猜出 Args 就代表着 n 个参数。
在该定义紧挨着的下面，我们就能看到上面调用的 `handler_service` 方法：

```rust
pub(crate) fn handler_service<F, Args>(handler: F) -> BoxedHttpServiceFactory
where
    F: Handler<Args>,
    Args: FromRequest,
    F::Output: Responder,
{
    boxed::factory(fn_service(move |req: ServiceRequest| {
        let handler = handler.clone();

        async move {
            let (req, mut payload) = req.into_parts();

            let res = match Args::from_request(&req, &mut payload).await {
                Err(err) => HttpResponse::from_error(err),

                Ok(data) => handler
                    .call(data)
                    .await
                    .respond_to(&req)
                    .map_into_boxed_body(),
            };

            Ok(ServiceResponse::new(req, res))
        }
    }))
}
```

我们看 `Args` 的约束能看到，只需要实现 `FromRequest` 就可以拿来做参数。也就是说，实现了 `FromRequest` 你可以在 Handle 方法中的任意参数位置写上你要的参数名和类型。

举个例子：当你在参数中要拿到 Http 的 Method 的时候，你可以这样:

```rust
async fn hander(method: Method) -> HttpResponse {...}

/// 因为 actix-web 内部 Method 实现了 FromRequest
impl FromRequest for Method {
    type Error = Infallible;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        ok(req.method().clone())
    }
}
```

接着 `from_request` 之后，如果成功提取出参数，就调用 `handler.call` 方法执行你的接口，如果失败了，就会返回在 `FromRequest` 中定义的Error，所以你可以随意定义参数提取失败之后所返回的内容。

不过看到这里还是会有疑问，这个 `Args` 到底传入的是什么? 

让我们继续往下看，会看到一个宏定义：

```rust
macro_rules! factory_tuple ({ $($param:ident)* } => {
    impl<Func, Fut, $($param,)*> Handler<($($param,)*)> for Func
    where
        Func: Fn($($param),*) -> Fut + Clone + 'static,
        Fut: Future,
    {
        type Output = Fut::Output;
        type Future = Fut;

        #[inline]
        #[allow(non_snake_case)]
        fn call(&self, ($($param,)*): ($($param,)*)) -> Self::Future {
            (self)($($param,)*)
        }
    }
});

factory_tuple! {}
factory_tuple! { A }
factory_tuple! { A B }
factory_tuple! { A B C }
factory_tuple! { A B C D }
factory_tuple! { A B C D E }
factory_tuple! { A B C D E F }
factory_tuple! { A B C D E F G }
factory_tuple! { A B C D E F G H }
factory_tuple! { A B C D E F G H I }
factory_tuple! { A B C D E F G H I J }
factory_tuple! { A B C D E F G H I J K }
factory_tuple! { A B C D E F G H I J K L }
```

> 如果你不了解宏的话，可以看看 [The Book](https://doc.rust-lang.org/book/ch19-06-macros.html) 还有 [rust by example](https://doc.rust-lang.org/rust-by-example/macros.html) 这里不做赘述了。

可以看到这里一揽子实现了各个类型，数量范围 [0, 12]，也就是把 Args 为 (), (A), (A, B)... 等都实现了。所以当你的接口超过12个参数的时候就会报错（应该不会有接口超过12个参数吧？）。

回答上面的问题： `Args` 就是 一堆 `tuple`，个数从 0 到 12。

那么又有新的问题来了，在上面的例子中 Method 实现了 `FromRequest`，但是在 `handler_service` 方法中，`from_request` 只被调用了一次，不应该有几个参数就调用几次吗？

这时候就需要再次看源码了，在 [FromRequest](https://docs.rs/actix-web/latest/actix_web/trait.FromRequest.html#method.extract) 文档中点击右上角的 [[source]](https://docs.rs/actix-web/latest/src/actix_web/extract.rs.html#65-97) （确保你的文档打开的版本是4.1.0），找到 312 行，会发现一个隐藏的模块 `tuple_from_req`，代码有点多，这里就只贴最关键的部分：

```rust
/// FromRequest implementation for tuple
#[allow(unused_parens)]
impl<$($T: FromRequest + 'static),+> FromRequest for ($($T,)+)
{
    type Error = Error;
    type Future = $fut<$($T),+>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        $fut {
            $(
                $T: ExtractFuture::Future {
                    fut: $T::from_request(req, payload)
                },
            )+
        }
    }
}
```

能看到这里，就已经水落石出了，原来是又用了个宏来帮tuple一个个调用 `from_request` 方法。所以在 `handler_service` 里的 `Args` 实际上是一个 `tuple`, 调用的 `from_request` 方法是从这里开始。

## 身份拦截的实现

简单定义一个用户数据，包含一个字段，代表用户id。

```rust
pub struct UserData {
    pub id: i32,
}
```

在我们登陆之后需要拿到拦截未登陆的用户的时候只需要在参数上写上它就好了：

```rust
async fn get_info(user: UserData) -> impl Responder {
    HttpResponse::Ok().finish()
}
```

当然，现在这样还不会生效，需要为 `UserData` 实现一下 `FromRequest`, 我们以 JWT 作为验证，因为不是重点，这里先忽略它吧，完整的代码会放在开源仓库上。

```rust
use actix_web::{dev::Payload, error, Error, FromRequest, HttpRequest};
use std::future::{ready, Ready};
impl FromRequest for UserData {
    type Error = Error;

    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready({
            let auth = req.headers().get("Authorization");
            if let Some(val) = auth {
                let token = val.to_str().unwrap().split("Bearer ").collect::<Vec<&str>>().pop().unwrap();
                let result = auth::validate_token(token);
                match result {
                    Ok(data) => Ok(UserData { id: data.claims.id }),
                    Err(e) => {
                        eprintln!("{}", e);
                        Err(error::ErrorBadRequest("Invalid Authorization"))
                    }
                }
            } else {
                Err(error::ErrorUnauthorized("Authorization Not Found"))
            }
        })
    }
}
```

实现了这个，刚才的接口就没问题了。

当我们想要登陆和未登陆的时候都能访问一个接口，但是接口返回的内容有差别，也可以利用这个方法来实现:

```rust
/// 无需登陆
/// 登陆前后拿到的数据不完全相同
async fn get_public_info(user: Option<UserData>) -> impl Responder {
    if let Some(user) = user {
        HttpResponse::Ok().json(format!("public data with {}", user.id))
    } else {
        HttpResponse::Ok().json("public data")
    }
}
```

这是因为在 actix-web 内部，帮你实现了 `Option<T: FromRequest>` 的情况。

另外，当你在接口中不需要使用到用户id，但仍希望做拦截，那么很简单，依旧把参数写上去，不使用就好了。

怎么样，是不是很灵活呢？ ：）

## 接口测试

### 登陆

```sh
# 登陆用户名为 123 密码为 456 的账号，返回值: "Bearer ..."
curl http://127.0.0.1:8000/login -H "Content-Type:application/json" -d '{"id":123,"pwd":"456"}'

# 导出返回值到环境变量以便后续使用
export token="Bearer ..."
```

### 获取信息

```sh
# 携带token，返回值为200
curl http://127.0.0.1:8000/info -v -X POST -H "Authorization:$token"

# Headers 不包含 Authorization 返回 401 Unauthorized 未验证
curl http://127.0.0.1:8000/info -v -X POST

# 输入错误的token，返回 400 Bad Request 返回 错误请求
curl http://127.0.0.1:8000/info -v -X POST -H "Authorization:Bearer ErrorToken"

```

### 获取公开信息

```sh
# 未携带token请求，返回公共数据：“public data" 不包含个人信息。
curl http://127.0.0.1:8000/public -v -X POST

# 携带了正确的token，返回带有个人数据的公共数据 ”public data with 123“
curl http://127.0.0.1:8000/public -v -X POST -H "Authorization:$token"
```
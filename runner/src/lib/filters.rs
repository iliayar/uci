use warp::Filter;

use super::handlers;


pub fn runner() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
   ping() 
}


pub fn ping() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("ping")
        .and(warp::get())
        .and_then(handlers::ping)
}

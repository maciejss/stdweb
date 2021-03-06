use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::fmt;
use std::error;
use webcore::try_from::{TryFrom, TryInto};
use webcore::number::{self, Number};

/// A unit type representing JavaScript's `undefined`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct Undefined;

/// A unit type representing JavaScript's `null`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Debug)]
pub struct Null;

/// A type representing a reference to a JavaScript value.
#[repr(C)]
#[derive(PartialEq, Eq, Debug)]
pub struct Reference( i32 );

impl Reference {
    #[doc(hidden)]
    #[inline]
    pub unsafe fn from_raw_unchecked( refid: i32 ) -> Reference {
        em_asm_int!( "Module.STDWEB.increment_refcount( $0 );", refid );
        Reference( refid )
    }

    #[doc(hidden)]
    #[inline]
    pub fn as_raw( &self ) -> i32 {
        self.0
    }

    /// Converts this reference into the given type `T`; checks whenever the reference
    /// is really of type `T` and returns `None` if it's not.
    #[inline]
    pub fn downcast< T: FromReference >( self ) -> Option< T > {
        T::from_reference( self )
    }
}

impl Clone for Reference {
    #[inline]
    fn clone( &self ) -> Self {
        unsafe {
            Reference::from_raw_unchecked( self.as_raw() )
        }
    }
}

impl Drop for Reference {
    #[inline]
    fn drop( &mut self ) {
        em_asm_int!( "Module.STDWEB.decrement_refcount( $0 );", self.0 );
    }
}

impl AsRef< Reference > for Reference {
    #[inline]
    fn as_ref( &self ) -> &Self {
        self
    }
}

macro_rules! __impl_infallible_try_from {
    (($($impl_arg:tt)*) ($($src_arg:tt)*) ($($dst_arg:tt)*) ($($bounds:tt)*)) => {
        impl< $($impl_arg)* > TryFrom< $($src_arg)* > for $($dst_arg)* where $($bounds)* {
            type Error = $crate::unstable::Void;

            #[inline]
            fn try_from( source: $($src_arg)* ) -> Result< Self, Self::Error > {
                Ok( source.into() )
            }
        }
    };
}

macro_rules! impl_infallible_try_from {
    (impl< $($impl_arg:tt),* > for $src:ty => $dst:ty where ($($bounds:tt)*); $($rest:tt)*) => {
        __impl_infallible_try_from!( ($($impl_arg),*) ($src) ($dst) ($($bounds)*) );
        impl_infallible_try_from!( $($rest)* );
    };

    (impl< $($impl_arg:tt),* > for $src:ty => $dst:ty; $($rest:tt)*) => {
        __impl_infallible_try_from!( ($($impl_arg),*) ($src) ($dst) () );
        impl_infallible_try_from!( $($rest)* );
    };

    ($src:ty => $dst:ty; $($rest:tt)*) => {
        __impl_infallible_try_from!( () ($src) ($dst) () );
        impl_infallible_try_from!( $($rest)* );

    };

    () => {};
}

impl_infallible_try_from! {
    Reference => Reference;
    impl< 'a > for &'a Reference => &'a Reference;
}

#[doc(hidden)]
pub trait FromReferenceUnchecked: Sized {
    unsafe fn from_reference_unchecked( reference: Reference ) -> Self;

    #[inline]
    unsafe fn from_value_unchecked( value: Value ) -> Option< Self > {
        let reference: Option< Reference > = value.try_into().ok();
        reference.map( |reference| Self::from_reference_unchecked( reference ) )
    }
}

#[doc(hidden)]
pub trait FromReference: FromReferenceUnchecked {
    fn from_reference( reference: Reference ) -> Option< Self >;
}

impl FromReferenceUnchecked for Reference {
    #[inline]
    unsafe fn from_reference_unchecked( reference: Reference ) -> Self {
        reference
    }
}

/// A type representing a JavaScript value.
///
/// This type implements a rich set of conversions
/// from and into standard Rust types, for example:
///
/// ```rust
/// let v1: Value = "Hello world!".into();
/// let v2: Value = true.into();
/// let v3: Value = vec![ 1, 2, 3 ].into();
/// let v4: Value = Null.into();
/// let v5: Value = 123_u64.try_into().unwrap();
///
/// let v1_r: String = v1.try_into().unwrap();
/// let v2_r: bool = v2.try_into().unwrap();
/// let v3_r: Vec< i32 > = v3.try_into().unwrap();
/// let v4_r: Option< String > = v4.try_into().unwrap(); // Will be `None`.
/// let v5_r: u64 = v5.try_into().unwrap();
/// ```
#[allow(missing_docs)]
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Undefined,
    Null,
    Bool( bool ),
    Number( Number ),
    String( String ),
    Array( Vec< Value > ),
    Object( BTreeMap< String, Value > ), // TODO: Use our own type instead of using BTreeMap directly.
    Reference( Reference )
}

impl Value {
    /// Checks whenever the Value is of the Reference variant.
    #[inline]
    pub fn is_reference( &self ) -> bool {
        if let Value::Reference( _ ) = *self {
            true
        } else {
            false
        }
    }

    /// Gets a reference to the [Reference](struct.Reference.html) inside this `Value`.
    #[inline]
    pub fn as_reference( &self ) -> Option< &Reference > {
        match *self {
            Value::Reference( ref reference ) => Some( reference ),
            _ => None
        }
    }

    /// Returns the [Reference](struct.Reference.html) inside this `Value`.
    #[inline]
    pub fn into_reference( self ) -> Option< Reference > {
        match self {
            Value::Reference( reference ) => Some( reference ),
            _ => None
        }
    }

    /// Converts a [Reference](struct.Reference.html) inside this `Value` into
    /// the given type `T`; doesn't check whenever the reference is really of type `T`.
    ///
    /// In cases where the value is not a `Reference` a `None` is returned.
    #[inline]
    pub unsafe fn into_reference_unchecked< T: FromReferenceUnchecked >( self ) -> Option< T > {
        T::from_value_unchecked( self )
    }

    /// Returns the `String` inside this `Value`.
    #[inline]
    pub fn into_string( self ) -> Option< String > {
        match self {
            Value::String( string ) => Some( string ),
            _ => None
        }
    }

    /// Returns a borrow of the string inside this `Value`.
    #[inline]
    pub fn as_str( &self ) -> Option< &str > {
        match *self {
            Value::String( ref string ) => Some( string.as_str() ),
            _ => None
        }
    }
}

impl AsRef< Value > for Value {
    #[inline]
    fn as_ref( &self ) -> &Self {
        self
    }
}

impl From< Undefined > for Value {
    #[inline]
    fn from( _: Undefined ) -> Self {
        Value::Undefined
    }
}

impl< 'a > From< &'a Undefined > for Value {
    #[inline]
    fn from( _: &'a Undefined ) -> Self {
        Value::Undefined
    }
}

impl< 'a > From< &'a mut Undefined > for Value {
    #[inline]
    fn from( _: &'a mut Undefined ) -> Self {
        Value::Undefined
    }
}

impl From< Null > for Value {
    #[inline]
    fn from( _: Null ) -> Self {
        Value::Null
    }
}

impl< 'a > From< &'a Null > for Value {
    #[inline]
    fn from( _: &'a Null ) -> Self {
        Value::Null
    }
}

impl< 'a > From< &'a mut Null > for Value {
    #[inline]
    fn from( _: &'a mut Null ) -> Self {
        Value::Null
    }
}

impl From< bool > for Value {
    #[inline]
    fn from( value: bool ) -> Self {
        Value::Bool( value )
    }
}

impl< 'a > From< &'a bool > for Value {
    #[inline]
    fn from( value: &'a bool ) -> Self {
        Value::Bool( *value )
    }
}

impl< 'a > From< &'a mut bool > for Value {
    #[inline]
    fn from( value: &'a mut bool ) -> Self {
        (value as &bool).into()
    }
}

impl< 'a > From< &'a str > for Value {
    #[inline]
    fn from( value: &'a str ) -> Self {
        Value::String( value.to_string() )
    }
}

impl< 'a > From< &'a mut str > for Value {
    #[inline]
    fn from( value: &'a mut str ) -> Self {
        (value as &str).into()
    }
}

impl From< String > for Value {
    #[inline]
    fn from( value: String ) -> Self {
        Value::String( value )
    }
}

impl< 'a > From< &'a String > for Value {
    #[inline]
    fn from( value: &'a String ) -> Self {
        Value::String( value.clone() )
    }
}

impl< 'a > From< &'a mut String > for Value {
    #[inline]
    fn from( value: &'a mut String ) -> Self {
        (value as &String).into()
    }
}

impl From< char > for Value {
    #[inline]
    fn from( value: char ) -> Self {
        let mut buffer: [u8; 4] = [0; 4];
        let string = value.encode_utf8( &mut buffer );
        string.to_owned().into()
    }
}

impl< 'a > From< &'a char > for Value {
    #[inline]
    fn from( value: &'a char ) -> Self {
        (*value).into()
    }
}

impl< 'a > From< &'a mut char > for Value {
    #[inline]
    fn from( value: &'a mut char ) -> Self {
        (*value).into()
    }
}

impl< T: Into< Value > > From< Vec< T > > for Value {
    #[inline]
    fn from( value: Vec< T > ) -> Self {
        Value::Array( value.into_iter().map( |element| element.into() ).collect() )
    }
}

impl< 'a, T > From< &'a Vec< T > > for Value where &'a T: Into< Value > {
    #[inline]
    fn from( value: &'a Vec< T > ) -> Self {
        value[..].into()
    }
}

impl< 'a, T > From< &'a mut Vec< T > > for Value where &'a T: Into< Value > {
    #[inline]
    fn from( value: &'a mut Vec< T > ) -> Self {
        value[..].into()
    }
}

impl< 'a, T > From< &'a [T] > for Value where &'a T: Into< Value > {
    #[inline]
    fn from( value: &'a [T] ) -> Self {
        Value::Array( value.iter().map( |element| {
            element.into()
        }).collect() )
    }
}

impl< 'a, T > From< &'a mut [T] > for Value where &'a T: Into< Value > {
    #[inline]
    fn from( value: &'a mut [T] ) -> Self {
        (value as &[T]).into()
    }
}

// TODO: It would be nice to specialize this for values which are already of type Value.
impl< K: Into< String >, V: Into< Value > > From< BTreeMap< K, V > > for Value {
    #[inline]
    fn from( value: BTreeMap< K, V > ) -> Self {
        let value = value.into_iter().map( |(key, value)| (key.into(), value.into()) ).collect();
        Value::Object( value )
    }
}

impl< 'a, K, V > From< &'a BTreeMap< K, V > > for Value where &'a K: Into< String >, &'a V: Into< Value > {
    #[inline]
    fn from( value: &'a BTreeMap< K, V > ) -> Self {
        let value = value.iter().map( |(key, value)| (key.into(), value.into()) ).collect();
        Value::Object( value )
    }
}

impl< 'a, K, V > From< &'a mut BTreeMap< K, V > > for Value where &'a K: Into< String >, &'a V: Into< Value > {
    #[inline]
    fn from( value: &'a mut BTreeMap< K, V > ) -> Self {
        let value: &BTreeMap< K, V > = value;
        value.into()
    }
}

impl< K: Into< String > + Hash + Eq, V: Into< Value > > From< HashMap< K, V > > for Value {
    #[inline]
    fn from( value: HashMap< K, V > ) -> Self {
        let value = value.into_iter().map( |(key, value)| (key.into(), value.into()) ).collect();
        Value::Object( value )
    }
}

impl< 'a, K: Hash + Eq, V > From< &'a HashMap< K, V > > for Value where &'a K: Into< String >, &'a V: Into< Value > {
    #[inline]
    fn from( value: &'a HashMap< K, V > ) -> Self {
        let value = value.iter().map( |(key, value)| (key.into(), value.into()) ).collect();
        Value::Object( value )
    }
}

impl< 'a, K: Hash + Eq, V > From< &'a mut HashMap< K, V > > for Value where &'a K: Into< String >, &'a V: Into< Value > {
    #[inline]
    fn from( value: &'a mut HashMap< K, V > ) -> Self {
        let value: &HashMap< K, V > = value;
        value.into()
    }
}

impl From< Reference > for Value {
    #[inline]
    fn from( value: Reference ) -> Self {
        Value::Reference( value )
    }
}

impl< 'a > From< &'a Reference > for Value {
    #[inline]
    fn from( value: &'a Reference ) -> Self {
        Value::Reference( value.clone() )
    }
}

impl< 'a > From< &'a mut Reference > for Value {
    #[inline]
    fn from( value: &'a mut Reference ) -> Self {
        (value as &Reference).into()
    }
}

macro_rules! impl_from_number {
    ($($kind:ty)+) => {
        $(
            impl From< $kind > for Value {
                #[inline]
                fn from( value: $kind ) -> Self {
                    Value::Number( value.into() )
                }
            }

            impl< 'a > From< &'a $kind > for Value {
                #[inline]
                fn from( value: &'a $kind ) -> Self {
                    Value::Number( (*value).into() )
                }
            }

            impl< 'a > From< &'a mut $kind > for Value {
                #[inline]
                fn from( value: &'a mut $kind ) -> Self {
                    (value as &$kind).into()
                }
            }

            impl_infallible_try_from!( $kind => Value; );
        )+
    };
}

impl_from_number!( i8 i16 i32 u8 u16 u32 f32 f64 );
impl_infallible_try_from! {
    Undefined => Value;
    impl< 'a > for &'a Undefined => Value;
    impl< 'a > for &'a mut Undefined => Value;
    Null => Value;
    impl< 'a > for &'a Null => Value;
    impl< 'a > for &'a mut Null => Value;
    bool => Value;
    impl< 'a > for &'a bool => Value;
    impl< 'a > for &'a mut bool => Value;
    impl< 'a > for &'a str => Value;
    impl< 'a > for &'a mut str => Value;
    String => Value;
    impl< 'a > for &'a String => Value;
    impl< 'a > for &'a mut String => Value;
    char => Value;
    impl< 'a > for &'a char => Value;
    impl< 'a > for &'a mut char => Value;
    impl< T > for Vec< T > => Value where (T: Into< Value >);
    impl< 'a, T > for &'a Vec< T > => Value where (&'a T: Into< Value >);
    impl< 'a, T > for &'a mut Vec< T > => Value where (&'a T: Into< Value >);
    impl< 'a, T > for &'a [T] => Value where (&'a T: Into< Value >);
    impl< 'a, T > for &'a mut [T] => Value where (&'a T: Into< Value >);
    impl< K, V > for BTreeMap< K, V > => Value where (K: Into< String >, V: Into< Value >);
    impl< 'a, K, V > for &'a BTreeMap< K, V > => Value where (&'a K: Into< String >, &'a V: Into< Value >);
    impl< 'a, K, V > for &'a mut BTreeMap< K, V > => Value where (&'a K: Into< String >, &'a V: Into< Value >);
    impl< K, V > for HashMap< K, V > => Value where (K: Into< String > + Hash + Eq, V: Into< Value >);
    impl< 'a, K, V > for &'a HashMap< K, V > => Value where (K: Hash + Eq, &'a K: Into< String >, &'a V: Into< Value >);
    impl< 'a, K, V > for &'a mut HashMap< K, V > => Value where (K: Hash + Eq, &'a K: Into< String >, &'a V: Into< Value >);
    Reference => Value;
}

macro_rules! impl_try_from_number {
    ($($kind:ty)+) => {
        $(
            impl TryFrom< $kind > for Value {
                type Error = <Number as TryFrom< $kind >>::Error;

                #[inline]
                fn try_from( value: $kind ) -> Result< Self, Self::Error > {
                    Ok( Value::Number( value.try_into()? ) )
                }
            }
        )+
    };
}

impl_try_from_number!( i64 u64 );

impl PartialEq< Undefined > for Value {
    #[inline]
    fn eq( &self, _: &Undefined ) -> bool {
        match *self {
            Value::Undefined => true,
            _ => false
        }
    }
}

impl PartialEq< Null > for Value {
    #[inline]
    fn eq( &self, _: &Null ) -> bool {
        match *self {
            Value::Null => true,
            _ => false
        }
    }
}

impl PartialEq< bool > for Value {
    #[inline]
    fn eq( &self, right: &bool ) -> bool {
        match *self {
            Value::Bool( left ) => left == *right,
            _ => false
        }
    }
}

impl PartialEq< str > for Value {
    #[inline]
    fn eq( &self, right: &str ) -> bool {
        match *self {
            Value::String( ref left ) => left == right,
            _ => false
        }
    }
}

impl PartialEq< String > for Value {
    #[inline]
    fn eq( &self, right: &String ) -> bool {
        match *self {
            Value::String( ref left ) => left == right,
            _ => false
        }
    }
}

impl< T > PartialEq< [T] > for Value where Value: PartialEq< T > {
    #[inline]
    fn eq( &self, right: &[T] ) -> bool {
        match *self {
            Value::Array( ref left ) => left.iter().zip( right.iter() ).all( |(left, right)| left == right ),
            _ => false
        }
    }
}

impl< 'a, T > PartialEq< &'a [T] > for Value where Value: PartialEq< T > {
    #[inline]
    fn eq( &self, right: &&'a [T] ) -> bool {
        <Self as PartialEq< [T] >>::eq( self, right )
    }
}

impl PartialEq< Number > for Value {
    #[inline]
    fn eq( &self, right: &Number ) -> bool {
        match *self {
            Value::Number( left ) => left == *right,
            _ => false
        }
    }
}

impl< T: AsRef< Reference > > PartialEq< T > for Value {
    #[inline]
    fn eq( &self, right: &T ) -> bool {
        match *self {
            Value::Reference( ref left ) => left == right.as_ref(),
            _ => false
        }
    }
}

impl< 'a > PartialEq< Reference > for &'a Value {
    #[inline]
    fn eq( &self, right: &Reference ) -> bool {
        (*self).eq( right )
    }
}

impl PartialEq< Value > for Reference {
    #[inline]
    fn eq( &self, right: &Value ) -> bool {
        right.eq( self )
    }
}

impl< 'a > PartialEq< &'a Value > for Reference {
    #[inline]
    fn eq( &self, right: &&'a Value ) -> bool {
        let right: &'a Value = right;
        right.eq( self )
    }
}

impl< 'a > PartialEq< Value > for &'a Reference {
    #[inline]
    fn eq( &self, right: &Value ) -> bool {
        (*self).eq( right )
    }
}

macro_rules! impl_partial_eq_boilerplate {
    ( $( $kind:ty ),+ ) => {
        $(
            impl< 'a > PartialEq< &'a $kind > for Value {
                #[inline]
                fn eq( &self, right: &&'a $kind ) -> bool {
                    let right: &'a $kind = right;
                    self.eq( right )
                }
            }

            impl< 'a > PartialEq< $kind > for &'a Value {
                #[inline]
                fn eq( &self, right: &$kind ) -> bool {
                    (*self).eq( right )
                }
            }

            impl PartialEq< Value > for $kind {
                #[inline]
                fn eq( &self, right: &Value ) -> bool {
                    right == self
                }
            }

            impl< 'a > PartialEq< &'a Value > for $kind {
                #[inline]
                fn eq( &self, right: &&'a Value ) -> bool {
                    let right: &'a Value = right;
                    right == self
                }
            }

            impl< 'a > PartialEq< Value > for &'a $kind {
                #[inline]
                fn eq( &self, right: &Value ) -> bool {
                    (*self).eq( right )
                }
            }
        )+
    }
}

macro_rules! impl_partial_eq_to_number {
    ($($kind:ty)+) => {
        $(
            impl PartialEq< $kind > for Value {
                #[inline]
                fn eq( &self, right: &$kind ) -> bool {
                    match *self {
                        Value::Number( left ) => left == *right,
                        _ => false
                    }
                }
            }

            impl_partial_eq_boilerplate!( $kind );
        )+
    };
}

impl_partial_eq_to_number!( i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 );

impl_partial_eq_boilerplate! {
    Undefined,
    Null,
    bool,
    str,
    String,
    Number
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConversionError {
    TypeMismatch {
        actual_type: &'static str
    },
    NumericConversionError( number::ConversionError ),
    ValueConversionError( Box< ConversionError > )
}

fn value_type_name( value: &Value ) -> &'static str {
    match *value {
        Value::Undefined => "Undefined",
        Value::Null => "Null",
        Value::Bool( _ ) => "Bool",
        Value::Number( _ ) => "Number",
        Value::String( _ ) => "String",
        Value::Array( _ ) => "Array",
        Value::Object( _ ) => "Object",
        Value::Reference( _ ) => "Reference"
    }
}

impl fmt::Display for ConversionError {
    fn fmt( &self, formatter: &mut fmt::Formatter ) -> Result< (), fmt::Error > {
        match *self {
            ConversionError::TypeMismatch { actual_type } => write!( formatter, "type mismatch; actual type is {}", actual_type ),
            ConversionError::NumericConversionError( ref inner ) => write!( formatter, "{}", inner ),
            ConversionError::ValueConversionError( ref inner ) => write!( formatter, "value conversion error: {}", inner )
        }
    }
}

impl error::Error for ConversionError {
    fn description( &self ) -> &str {
        match *self {
            ConversionError::TypeMismatch { .. } => "type mismatch",
            ConversionError::NumericConversionError( ref inner ) => inner.description(),
            ConversionError::ValueConversionError( _ ) => "value conversion error"
        }
    }
}

impl From< number::ConversionError > for ConversionError {
    fn from( inner: number::ConversionError ) -> Self {
        ConversionError::NumericConversionError( inner )
    }
}

impl ConversionError {
    #[inline]
    fn type_mismatch( actual_value: &Value ) -> Self {
        ConversionError::TypeMismatch {
            actual_type: value_type_name( actual_value )
        }
    }

    #[inline]
    fn value_conversion_error( inner: ConversionError ) -> Self {
        ConversionError::ValueConversionError( Box::new( inner ) )
    }
}

impl TryFrom< Value > for Undefined {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: Value ) -> Result< Self, Self::Error > {
        match value {
            Value::Undefined => Ok( Undefined ),
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

impl TryFrom< Value > for Null {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: Value ) -> Result< Self, Self::Error > {
        match value {
            Value::Null => Ok( Null ),
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

impl TryFrom< Value > for bool {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: Value ) -> Result< Self, Self::Error > {
        match value {
            Value::Bool( value ) => Ok( value ),
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

macro_rules! impl_try_into_number {
    ($($kind:ty)+) => {
        $(
            impl TryFrom< Value > for $kind {
                type Error = ConversionError;

                #[inline]
                fn try_from( value: Value ) -> Result< Self, Self::Error > {
                    match value {
                        Value::Number( value ) => {
                            let result: Result< Self, _ > = value.try_into();
                            result.map_err( |error| error.into() )
                        },
                        _ => Err( ConversionError::type_mismatch( &value ) )
                    }
                }
            }
        )+
    };
}

impl_try_into_number!( u8 u16 u32 u64 i8 i16 i32 i64 f64 );

impl< V: TryFrom< Value, Error = ConversionError > > TryFrom< Value > for BTreeMap< String, V > {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: Value ) -> Result< Self, Self::Error > {
        match value {
            Value::Object( object ) => {
                let mut output = BTreeMap::new();
                for (key, value) in object {
                    let value = match value.try_into() {
                        Ok( value ) => value,
                        Err( error ) => return Err( ConversionError::value_conversion_error( error ) )
                    };
                    output.insert( key, value );
                }
                Ok( output )
            },
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

impl< V: TryFrom< Value, Error = ConversionError > > TryFrom< Value > for HashMap< String, V > {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: Value ) -> Result< Self, Self::Error > {
        match value {
            Value::Object( object ) => {
                let mut output = HashMap::with_capacity( object.len() );
                for (key, value) in object {
                    let value = match value.try_into() {
                        Ok( value ) => value,
                        Err( error ) => return Err( ConversionError::value_conversion_error( error ) )
                    };
                    output.insert( key, value );
                }
                Ok( output )
            },
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

impl< T: TryFrom< Value, Error = ConversionError > > TryFrom< Value > for Vec< T > {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: Value ) -> Result< Self, Self::Error > {
        match value {
            Value::Array( array ) => {
                let mut output = Vec::with_capacity( array.len() );
                for value in array {
                    let value = match value.try_into() {
                        Ok( value ) => value,
                        Err( error ) => return Err( ConversionError::value_conversion_error( error ) )
                    };
                    output.push( value );
                }
                Ok( output )
            },
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

impl TryFrom< Value > for String {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: Value ) -> Result< Self, Self::Error > {
        match value {
            Value::String( value ) => Ok( value ),
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

impl TryFrom< Value > for Reference {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: Value ) -> Result< Self, Self::Error > {
        match value {
            Value::Reference( value ) => Ok( value ),
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

impl< 'a > TryFrom< &'a Value > for &'a str {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: &'a Value ) -> Result< Self, Self::Error > {
        match *value {
            Value::String( ref value ) => Ok( value ),
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

impl< 'a > TryFrom< &'a Value > for &'a Reference {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: &'a Value ) -> Result< Self, Self::Error > {
        match *value {
            Value::Reference( ref value ) => Ok( value ),
            _ => Err( ConversionError::type_mismatch( &value ) )
        }
    }
}

macro_rules! __impl_nullable_try_from_value {
    (($($impl_arg:tt)*) ($($dst_arg:tt)*) ($($bounds:tt)*)) => {
        impl< $($impl_arg)* > TryFrom< Value > for Option< $($dst_arg)* > where $($bounds)* {
            type Error = ConversionError;

            #[inline]
            fn try_from( value: Value ) -> Result< Self, Self::Error > {
                match value {
                    Value::Undefined | Value::Null => Ok( None ),
                    value => value.try_into().map( Some )
                }
            }
        }
    };
}

macro_rules! impl_nullable_try_from_value {
    (impl< $($impl_arg:tt),* > $dst:ty where ($($bounds:tt)*); $($rest:tt)*) => {
        __impl_nullable_try_from_value!( ($($impl_arg),*) ($dst) ($($bounds)*) );
        impl_nullable_try_from_value!( $($rest)* );
    };

    (impl< $($impl_arg:tt),* > $dst:ty; $($rest:tt)*) => {
        __impl_nullable_try_from_value!( ($($impl_arg),*) ($dst) () );
        impl_nullable_try_from_value!( $($rest)* );
    };

    ($dst:ty; $($rest:tt)*) => {
        __impl_nullable_try_from_value!( () ($dst) () );
        impl_nullable_try_from_value!( $($rest)* );

    };

    () => {};
}

impl_nullable_try_from_value! {
    bool;
    u8;
    u16;
    u32;
    u64;
    i8;
    i16;
    i32;
    i64;
    f64;
    impl< V > BTreeMap< String, V > where (V: TryFrom< Value, Error = ConversionError >);
    impl< V > HashMap< String, V > where (V: TryFrom< Value, Error = ConversionError >);
    impl< T > Vec< T > where (T: TryFrom< Value, Error = ConversionError >);
    String;
    Reference;
}

impl< 'a > TryFrom< &'a Value > for Option< &'a str > {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: &'a Value ) -> Result< Self, Self::Error > {
        match *value {
            Value::String( ref value ) => Ok( Some( value ) ),
            ref value => value.try_into().map( Some )
        }
    }
}

impl< 'a > TryFrom< &'a Value > for Option< &'a Reference > {
    type Error = ConversionError;

    #[inline]
    fn try_from( value: &'a Value ) -> Result< Self, Self::Error > {
        match *value {
            Value::Reference( ref value ) => Ok( Some( value ) ),
            ref value => value.try_into().map( Some )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, Reference};
    use webcore::try_from::TryInto;

    #[test]
    fn string_equality() {
        let value = Value::String( "Hello!".to_owned() );
        assert!( value == "Hello!" );
        assert!( &value == "Hello!" );
        assert!( value == "Hello!".to_owned() );
        assert!( &value == "Hello!".to_owned() );
        assert!( value == &"Hello!".to_owned() );
        assert!( &value == &"Hello!".to_owned() );
        assert!( "Hello!" == value );
        assert!( "Hello!" == &value );
        assert!( "Hello!".to_owned() == value );
        assert!( "Hello!".to_owned() == &value );
        assert!( &"Hello!".to_owned() == value );
        assert!( &"Hello!".to_owned() == &value );

        assert!( value != "Bob" );
    }

    #[test]
    fn array_equality() {
        let value = Value::Array( vec![ Value::Bool( true ), Value::Bool( false ) ] );
        assert!( value == &[true, false][..] );
        assert!( value != &[true, true][..] );
        // Looks like it's not possible to define a symmetric PartialEq for arrays. );
    }

    #[test]
    fn reference_equality() {
        let value = js! { return new Date() };
        let reference: Reference = value.clone().try_into().unwrap();

        assert!( value == reference );
        assert!( &value == reference );
        assert!( value == &reference );
        assert!( &value == &reference );
        assert!( reference == value );
        assert!( &reference == value );
        assert!( reference == &value );
        assert!( &reference == &value );
    }

    pub struct Error( Reference );
    reference_boilerplate! {
        Error,
        instanceof Error
    }

    pub struct ReferenceError( Reference );
    reference_boilerplate! {
        ReferenceError,
        instanceof ReferenceError
        convertible to Error
    }

    pub struct TypeError( Reference );
    reference_boilerplate! {
        TypeError,
        instanceof TypeError
        convertible to Error
    }

    #[test]
    fn reference_downcast() {
        let reference = js! { return new ReferenceError(); }.into_reference().unwrap();
        assert!( reference.clone().downcast::< Error >().is_some() );
        assert!( reference.clone().downcast::< ReferenceError >().is_some() );
        assert!( reference.clone().downcast::< TypeError >().is_none() );
    }

    #[test]
    fn reference_try_into_downcast_from_reference() {
        let reference = js! { return new ReferenceError(); }.into_reference().unwrap();
        let typed_reference: Result< Error, _ > = reference.clone().try_into();
        assert!( typed_reference.is_ok() );

        let typed_reference: Result< ReferenceError, _ > = reference.clone().try_into();
        assert!( typed_reference.is_ok() );

        let typed_reference: Result< TypeError, _ > = reference.clone().try_into();
        assert!( typed_reference.is_err() );
    }

    #[test]
    fn reference_try_into_downcast_from_value() {
        let value = js! { return new ReferenceError(); };
        let typed_reference: Result< Error, _ > = value.clone().try_into();
        assert!( typed_reference.is_ok() );

        let typed_reference: Result< ReferenceError, _ > = value.clone().try_into();
        assert!( typed_reference.is_ok() );

        let typed_reference: Result< TypeError, _ > = value.clone().try_into();
        assert!( typed_reference.is_err() );
    }

    #[test]
    fn reference_into_upcast() {
        let reference: ReferenceError = js! { return new ReferenceError(); }.into_reference().unwrap().downcast().unwrap();
        let _: Error = reference.clone().into();
        let _: Reference = reference.clone().into();
    }
}
